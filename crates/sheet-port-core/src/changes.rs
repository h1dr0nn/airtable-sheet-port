//! Pending-change lifecycle: preview -> commit, with an optional reject.
//! There is no human approval gate: commit auto-approves a pending change by
//! policy after re-checking permissions. The desktop app can still reject a
//! staged change in the shared DB, and the broker reads fresh state at commit
//! time, so a rejection applies across processes without direct IPC.
//! Every error message here is part of the observable contract.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audit;
use crate::connectors::ConnectorRegistry;
use crate::constants::{BULK_UPDATE_THRESHOLD, CHANGE_LIST_LIMIT};
use crate::db::now_iso;
use crate::error::{db_error, parse_json, CoreError};
use crate::permissions;
use crate::types::{
    AuditActor, CellWrite, ChangeDecider, ChangeStatus, ChangeType, CreatedResource, FormatPlan,
    JsonMap, PendingChange, ReadOptions, RecordPatch, TableRecord, WriteAction,
};

/// Internal write payload; persisted alongside a change but never returned to
/// agents or the desktop frontend. JSON shape matches the TypeScript
/// reference: {"type":"append","records":[...]}, {"type":"update",
/// "patches":[...]}, {"type":"delete","recordIds":[...]},
/// {"type":"format","plan":{...}}.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ChangePayload {
    Append {
        records: Vec<JsonMap>,
        /// Optional formatting applied in the same commit, right after the rows
        /// land, so a fresh table can be written and styled in one call. Absent
        /// on plain appends.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        format: Option<FormatPlan>,
    },
    Update {
        patches: Vec<RecordPatch>,
    },
    #[serde(rename_all = "camelCase")]
    Delete {
        record_ids: Vec<String>,
    },
    Format {
        plan: FormatPlan,
    },
    /// Coordinate-level cell writes (A1 addressed, USER_ENTERED). The escape
    /// hatch for document-style sheets the record model cannot address.
    UpdateCells {
        cells: Vec<CellWrite>,
    },
    /// Create a brand-new spreadsheet (source-level; the change's table_id is
    /// empty). Commit returns the new spreadsheet id/url.
    CreateSpreadsheet {
        title: String,
    },
    /// Add a new sheet tab to the spreadsheet named by the change's table_id.
    /// Commit returns the new tab's gid.
    CreateSheet {
        title: String,
    },
    /// Delete the sheet tab named by the change's table_id. Gated on the
    /// `delete_records` permission.
    DeleteSheet {},
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitOutcome {
    pub change: PendingChange,
    pub records: Vec<TableRecord>,
    /// Set only when a bundled append+format commit wrote the rows but the
    /// follow-up formatting failed. The rows are committed (so re-committing
    /// would duplicate them); the styling can be retried with
    /// format_table. Absent on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_error: Option<String>,
    /// Set when the committed change created a resource (a new spreadsheet or
    /// sheet tab), carrying its id/url so the agent can address it next.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<CreatedResource>,
}

// The internal `payload` column is intentionally absent: it must never reach
// agents or the desktop frontend. The legacy `requires_confirmation` column is
// no longer read; new rows always store 0 there.
const CHANGE_COLUMNS: &str = "id, source_id, table_id, change_type, created_at, status, \
     diff, decided_at, decided_by, committed_at";

// ---------------------------------------------------------------------------
// Previews
// ---------------------------------------------------------------------------

pub fn create_append_change(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    records: Vec<JsonMap>,
) -> Result<PendingChange, CoreError> {
    create_append_with_format(conn, source_id, table_id, records, None)
}

/// Stages an append that optionally carries a formatting plan applied in the
/// same commit (see [`ChangePayload::Append`]). The diff shows the appended
/// rows and, when present, the format plan so the change is fully previewable.
pub fn create_append_with_format(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    records: Vec<JsonMap>,
    format: Option<FormatPlan>,
) -> Result<PendingChange, CoreError> {
    let mut diff = serde_json::json!({ "after": records });
    if let Some(plan) = &format {
        diff["format"] = serde_json::to_value(plan).map_err(|error| {
            CoreError::Storage(format!("Could not encode format plan: {error}"))
        })?;
    }
    insert_change(
        conn,
        source_id,
        table_id,
        ChangeType::Append,
        &ChangePayload::Append { records, format },
        diff,
    )
}

/// Builds a per-record before/after diff by reading the current records
/// through the connector; unknown record ids diff against `null`.
pub fn create_update_change(
    conn: &Connection,
    registry: &ConnectorRegistry,
    source_id: &str,
    table_id: &str,
    patches: Vec<RecordPatch>,
) -> Result<PendingChange, CoreError> {
    let current = registry.read_table(conn, source_id, table_id, ReadOptions::default())?;
    let diff = Value::Array(
        patches
            .iter()
            .map(|patch| build_update_diff_entry(&current, patch))
            .collect(),
    );
    insert_change(
        conn,
        source_id,
        table_id,
        ChangeType::Update,
        &ChangePayload::Update { patches },
        diff,
    )
}

/// Stages a formatting change. The plan is safe to expose, so it doubles as the
/// agent-visible diff; the same plan is stored as the internal payload the
/// commit replays through the connector.
pub fn create_format_change(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    plan: FormatPlan,
) -> Result<PendingChange, CoreError> {
    let diff = serde_json::to_value(&plan)
        .map_err(|error| CoreError::Storage(format!("Could not encode format plan: {error}")))?;
    insert_change(
        conn,
        source_id,
        table_id,
        ChangeType::Format,
        &ChangePayload::Format { plan },
        diff,
    )
}

/// Stages coordinate-level cell writes. The diff lists each targeted cell with
/// the value that will be typed into it, so the change is fully previewable
/// without reading the sheet.
pub fn create_update_cells_change(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    cells: Vec<CellWrite>,
) -> Result<PendingChange, CoreError> {
    let diff = serde_json::json!({
        "cells": cells
            .iter()
            .map(|write| serde_json::json!({ "cell": write.a1(), "value": write.value }))
            .collect::<Vec<_>>()
    });
    insert_change(
        conn,
        source_id,
        table_id,
        ChangeType::UpdateCells,
        &ChangePayload::UpdateCells { cells },
        diff,
    )
}

/// Stages the creation of a brand-new spreadsheet. Source-level: the change's
/// table_id is empty and the permission check resolves against the source-wide
/// rule.
pub fn create_create_spreadsheet_change(
    conn: &Connection,
    source_id: &str,
    title: String,
) -> Result<PendingChange, CoreError> {
    let diff = serde_json::json!({ "title": title });
    insert_change(
        conn,
        source_id,
        "",
        ChangeType::CreateSpreadsheet,
        &ChangePayload::CreateSpreadsheet { title },
        diff,
    )
}

/// Stages adding a new sheet tab to the spreadsheet named by `table_id`.
pub fn create_create_sheet_change(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    title: String,
) -> Result<PendingChange, CoreError> {
    let diff = serde_json::json!({ "title": title, "spreadsheet": table_id });
    insert_change(
        conn,
        source_id,
        table_id,
        ChangeType::CreateSheet,
        &ChangePayload::CreateSheet { title },
        diff,
    )
}

/// Stages deleting the sheet tab named by `table_id`.
pub fn create_delete_sheet_change(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
) -> Result<PendingChange, CoreError> {
    let diff = serde_json::json!({ "deleteSheet": table_id });
    insert_change(
        conn,
        source_id,
        table_id,
        ChangeType::DeleteSheet,
        &ChangePayload::DeleteSheet {},
        diff,
    )
}

fn build_update_diff_entry(current: &[TableRecord], patch: &RecordPatch) -> Value {
    let before = current
        .iter()
        .find(|record| record.id == patch.record_id)
        .map(|record| &record.fields);
    let mut after = before.cloned().unwrap_or_default();
    for (key, value) in &patch.fields {
        after.insert(key.clone(), value.clone());
    }
    serde_json::json!({
        "recordId": patch.record_id,
        "before": before.map_or(Value::Null, |fields| Value::Object(fields.clone())),
        "after": after,
    })
}

fn insert_change(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    change_type: ChangeType,
    payload: &ChangePayload,
    diff: Value,
) -> Result<PendingChange, CoreError> {
    let change = PendingChange {
        id: format!("chg_{}", uuid::Uuid::new_v4()),
        source_id: source_id.to_string(),
        table_id: table_id.to_string(),
        change_type,
        created_at: now_iso(),
        status: ChangeStatus::Pending,
        diff,
        decided_at: None,
        decided_by: None,
        committed_at: None,
    };
    let diff_json = serde_json::to_string(&change.diff)
        .map_err(|error| CoreError::Storage(format!("Could not encode change diff: {error}")))?;
    let payload_json = serde_json::to_string(payload)
        .map_err(|error| CoreError::Storage(format!("Could not encode change payload: {error}")))?;
    conn.execute(
        "INSERT INTO pending_changes
             (id, source_id, table_id, change_type, created_at, status,
              requires_confirmation, diff, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            change.id,
            change.source_id,
            change.table_id,
            change.change_type.as_str(),
            change.created_at,
            change.status.as_str(),
            false,
            diff_json,
            payload_json
        ],
    )
    .map_err(|error| db_error("Could not insert pending change", error))?;
    Ok(change)
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

fn map_change(row: &Row<'_>) -> rusqlite::Result<RawChange> {
    Ok(RawChange {
        id: row.get(0)?,
        source_id: row.get(1)?,
        table_id: row.get(2)?,
        change_type: row.get(3)?,
        created_at: row.get(4)?,
        status: row.get(5)?,
        diff: row.get(6)?,
        decided_at: row.get(7)?,
        decided_by: row.get(8)?,
        committed_at: row.get(9)?,
    })
}

struct RawChange {
    id: String,
    source_id: String,
    table_id: String,
    change_type: String,
    created_at: String,
    status: String,
    diff: String,
    decided_at: Option<String>,
    decided_by: Option<String>,
    committed_at: Option<String>,
}

impl RawChange {
    fn finish(self) -> Result<PendingChange, CoreError> {
        let change_type = ChangeType::from_db(&self.change_type).ok_or_else(|| {
            CoreError::Storage(format!(
                "Change {} has unknown type '{}'",
                self.id, self.change_type
            ))
        })?;
        let status = ChangeStatus::from_db(&self.status).ok_or_else(|| {
            CoreError::Storage(format!(
                "Change {} has unknown status '{}'",
                self.id, self.status
            ))
        })?;
        let decided_by = self
            .decided_by
            .map(|raw| {
                ChangeDecider::from_db(&raw).ok_or_else(|| {
                    CoreError::Storage(format!("Change {} has unknown decider '{raw}'", self.id))
                })
            })
            .transpose()?;
        let diff = parse_json(&self.diff, &format!("Diff of change {}", self.id))?;
        Ok(PendingChange {
            id: self.id,
            source_id: self.source_id,
            table_id: self.table_id,
            change_type,
            created_at: self.created_at,
            status,
            diff,
            decided_at: self.decided_at,
            decided_by,
            committed_at: self.committed_at,
        })
    }
}

pub fn get_change(conn: &Connection, change_id: &str) -> Result<Option<PendingChange>, CoreError> {
    let sql = format!("SELECT {CHANGE_COLUMNS} FROM pending_changes WHERE id = ?1");
    let raw = conn
        .query_row(&sql, [change_id], map_change)
        .optional()
        .map_err(|error| db_error("Could not load change", error))?;
    raw.map(RawChange::finish).transpose()
}

/// The stored write payload; JSON `null` reads as `None` (parity with the
/// TypeScript store, where a nullish payload means "no stored payload").
pub fn get_payload(conn: &Connection, change_id: &str) -> Result<Option<ChangePayload>, CoreError> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT payload FROM pending_changes WHERE id = ?1",
            [change_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| db_error("Could not load change payload", error))?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    parse_json::<Option<ChangePayload>>(&raw, &format!("Payload of change {change_id}"))
}

/// Newest first, all statuses when the filter is omitted; capped at 200 rows.
pub fn list_changes(
    conn: &Connection,
    status: Option<&str>,
) -> Result<Vec<PendingChange>, CoreError> {
    let status = status
        .map(|raw| {
            ChangeStatus::from_db(raw).ok_or_else(|| {
                CoreError::InvalidInput(format!(
                    "Unknown change status '{raw}'. Allowed: {}",
                    ChangeStatus::ALL.map(ChangeStatus::as_str).join(", ")
                ))
            })
        })
        .transpose()?;

    let sql = format!(
        "SELECT {CHANGE_COLUMNS} FROM pending_changes
         WHERE (?1 IS NULL OR status = ?1)
         ORDER BY created_at DESC LIMIT {CHANGE_LIST_LIMIT}"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|error| db_error("Could not list changes", error))?;
    let rows = stmt
        .query_map(params![status.map(ChangeStatus::as_str)], map_change)
        .map_err(|error| db_error("Could not list changes", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list changes", error))?;
    rows.into_iter().map(RawChange::finish).collect()
}

// ---------------------------------------------------------------------------
// Transitions
// ---------------------------------------------------------------------------

/// Atomic guarded transition (UPDATE ... WHERE status = from); false when the
/// guard missed because another process changed the row first.
pub fn transition(
    conn: &Connection,
    change_id: &str,
    from: ChangeStatus,
    to: ChangeStatus,
    decided_by: ChangeDecider,
) -> Result<bool, CoreError> {
    let updated = conn
        .execute(
            "UPDATE pending_changes SET status = ?1, decided_at = ?2, decided_by = ?3
             WHERE id = ?4 AND status = ?5",
            params![
                to.as_str(),
                now_iso(),
                decided_by.as_str(),
                change_id,
                from.as_str()
            ],
        )
        .map_err(|error| db_error("Could not update change", error))?;
    Ok(updated == 1)
}

/// Atomic approved -> committed; false when the change was not approved.
pub fn mark_committed(conn: &Connection, change_id: &str) -> Result<bool, CoreError> {
    let updated = conn
        .execute(
            "UPDATE pending_changes SET status = 'committed', committed_at = ?1
             WHERE id = ?2 AND status = 'approved'",
            params![now_iso(), change_id],
        )
        .map_err(|error| db_error("Could not mark change committed", error))?;
    Ok(updated == 1)
}

/// Atomic policy-approved -> pending with the decision cleared; used when the
/// write fails after `commit` auto-approved the change.
fn revert_to_pending(conn: &Connection, change_id: &str) -> Result<bool, CoreError> {
    let updated = conn
        .execute(
            "UPDATE pending_changes SET status = 'pending', decided_at = NULL, decided_by = NULL
             WHERE id = ?1 AND status = 'approved' AND decided_by = 'policy'",
            [change_id],
        )
        .map_err(|error| db_error("Could not update change", error))?;
    Ok(updated == 1)
}

// ---------------------------------------------------------------------------
// Desktop decision (discard a staged change)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum ChangeDecision {
    Reject,
}

impl ChangeDecision {
    fn new_status(self) -> ChangeStatus {
        match self {
            Self::Reject => ChangeStatus::Rejected,
        }
    }

    fn audit_action(self) -> &'static str {
        match self {
            Self::Reject => "change_rejected",
        }
    }

    fn verb(self) -> &'static str {
        match self {
            Self::Reject => "rejected",
        }
    }
}

pub fn decide_change(
    conn: &Connection,
    change_id: &str,
    decision: ChangeDecision,
) -> Result<PendingChange, CoreError> {
    let transitioned = transition(
        conn,
        change_id,
        ChangeStatus::Pending,
        decision.new_status(),
        ChangeDecider::User,
    )?;
    if !transitioned {
        let current: Option<String> = conn
            .query_row(
                "SELECT status FROM pending_changes WHERE id = ?1",
                [change_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| db_error("Could not read change status", error))?;
        return Err(match current {
            Some(status) => CoreError::Conflict(format!(
                "Change {change_id} cannot be {}: its status is '{status}', not 'pending'",
                decision.verb()
            )),
            None => CoreError::NotFound(format!("Change {change_id} was not found")),
        });
    }

    let change = get_change(conn, change_id)?
        .ok_or_else(|| CoreError::NotFound(format!("Change {change_id} was not found")))?;
    audit::record(
        conn,
        AuditActor::User,
        decision.audit_action(),
        Some(&change.source_id),
        Some(&change.table_id),
        Some(&serde_json::json!({ "changeId": change_id })),
    )?;
    Ok(change)
}

// ---------------------------------------------------------------------------
// Commit
// ---------------------------------------------------------------------------

pub fn commit(
    conn: &Connection,
    registry: &ConnectorRegistry,
    change_id: &str,
) -> Result<CommitOutcome, CoreError> {
    let change = get_change(conn, change_id)?
        .ok_or_else(|| CoreError::NotFound(format!("Unknown change {change_id}")))?;
    if change.status == ChangeStatus::Rejected {
        return Err(CoreError::Conflict(format!(
            "Change {change_id} was rejected in the desktop app and cannot be committed"
        )));
    }
    if change.status == ChangeStatus::Committed {
        return Err(CoreError::Conflict(format!(
            "Change {change_id} is already committed"
        )));
    }

    let payload = get_payload(conn, change_id)?
        .ok_or_else(|| CoreError::Conflict(format!("Change {change_id} has no stored payload")))?;

    // Permission rules may have changed since preview; re-check at commit
    // time with the same action the preview evaluated.
    permissions::assert_can_write(
        conn,
        &change.source_id,
        &change.table_id,
        commit_action(change.change_type, &payload),
    )?;

    let approved_here = change.status == ChangeStatus::Pending;
    if approved_here {
        // No human approval gate: policy auto-approves a pending change right
        // before the write.
        let transitioned = transition(
            conn,
            change_id,
            ChangeStatus::Pending,
            ChangeStatus::Approved,
            ChangeDecider::Policy,
        )?;
        if !transitioned {
            let current = get_change(conn, change_id)?;
            let status = current
                .map(|change| change.status.as_str())
                .unwrap_or("missing");
            return Err(CoreError::Conflict(format!(
                "Change {change_id} is {status} and cannot be committed"
            )));
        }
    }

    let exec = match execute(conn, registry, &change, &payload) {
        Ok(exec) => exec,
        Err(error) => {
            // Put a change approved above back to pending so it stays staged:
            // the agent can retry it and the desktop app can still discard it
            // (reject only applies to pending changes). Best effort; the
            // write error is what the caller needs to see.
            if approved_here {
                let _ = revert_to_pending(conn, change_id);
            }
            return Err(error);
        }
    };
    if !mark_committed(conn, change_id)? {
        return Err(CoreError::Conflict(format!(
            "Change {change_id} could not be marked committed (state changed concurrently)"
        )));
    }
    let committed = get_change(conn, change_id)?.ok_or_else(|| {
        CoreError::Storage(format!("Change {change_id} disappeared after commit"))
    })?;
    // Bundled append+format: the rows are already committed above, so a failure
    // to apply the styling must NOT fail the commit (re-committing would
    // duplicate the rows). Surface it via `format_error` instead so the caller
    // can retry styling with format_table.
    let format_error = match &payload {
        ChangePayload::Append {
            format: Some(plan), ..
        } => registry
            .format_cells(conn, &change.source_id, &change.table_id, plan)
            .err()
            .map(|error| error.to_string()),
        _ => None,
    };
    Ok(CommitOutcome {
        change: committed,
        records: exec.records,
        format_error,
        created: exec.created,
    })
}

/// The result of executing a change's primary write: the affected records and,
/// for structural changes, the resource that was created.
struct ExecOutcome {
    records: Vec<TableRecord>,
    created: Option<CreatedResource>,
}

/// Commits several staged changes in order, returning one outcome per id. All
/// ids are checked to exist first so a typo fails before any write lands; after
/// that each change commits through [`commit`], so a mid-sequence failure leaves
/// the already-committed changes applied (there is no cross-request rollback in
/// the Sheets API).
pub fn commit_many(
    conn: &Connection,
    registry: &ConnectorRegistry,
    change_ids: &[String],
) -> Result<Vec<CommitOutcome>, CoreError> {
    for change_id in change_ids {
        if get_change(conn, change_id)?.is_none() {
            return Err(CoreError::NotFound(format!("Unknown change {change_id}")));
        }
    }
    change_ids
        .iter()
        .map(|change_id| commit(conn, registry, change_id))
        .collect()
}

fn execute(
    conn: &Connection,
    registry: &ConnectorRegistry,
    change: &PendingChange,
    payload: &ChangePayload,
) -> Result<ExecOutcome, CoreError> {
    let records_only = |records| ExecOutcome {
        records,
        created: None,
    };
    let outcome = match payload {
        // A bundled format plan is applied by `commit` AFTER the rows are marked
        // committed, so it is intentionally ignored here.
        ChangePayload::Append { records, .. } => records_only(registry.append_records(
            conn,
            &change.source_id,
            &change.table_id,
            records,
        )?),
        ChangePayload::Update { patches } => records_only(registry.update_records(
            conn,
            &change.source_id,
            &change.table_id,
            patches,
        )?),
        ChangePayload::Delete { .. } => {
            return Err(CoreError::Unsupported(
                "Delete changes are not implemented in the MVP".to_string(),
            ))
        }
        ChangePayload::Format { plan } => {
            registry.format_cells(conn, &change.source_id, &change.table_id, plan)?;
            records_only(Vec::new())
        }
        ChangePayload::UpdateCells { cells } => {
            registry.update_cells(conn, &change.source_id, &change.table_id, cells)?;
            // Cell writes are not record-shaped; the committed change carries none.
            records_only(Vec::new())
        }
        ChangePayload::CreateSpreadsheet { title } => ExecOutcome {
            records: Vec::new(),
            created: Some(registry.create_spreadsheet(conn, &change.source_id, title)?),
        },
        ChangePayload::CreateSheet { title } => ExecOutcome {
            records: Vec::new(),
            created: Some(registry.create_sheet(
                conn,
                &change.source_id,
                &change.table_id,
                title,
            )?),
        },
        ChangePayload::DeleteSheet {} => {
            registry.delete_sheet(conn, &change.source_id, &change.table_id)?;
            records_only(Vec::new())
        }
    };
    Ok(outcome)
}

/// Re-derives the evaluated action so commit re-checks the same policy the
/// preview used: large updates (record patches or cell writes) escalate to
/// bulk_update.
fn commit_action(change_type: ChangeType, payload: &ChangePayload) -> WriteAction {
    let over_bulk_threshold = match payload {
        ChangePayload::Update { patches } => patches.len() > BULK_UPDATE_THRESHOLD,
        ChangePayload::UpdateCells { cells } => cells.len() > BULK_UPDATE_THRESHOLD,
        _ => false,
    };
    if over_bulk_threshold {
        return WriteAction::BulkUpdate;
    }
    match change_type {
        ChangeType::Append => WriteAction::Append,
        ChangeType::Update => WriteAction::Update,
        ChangeType::Delete => WriteAction::Delete,
        ChangeType::Format => WriteAction::Format,
        // Cell writes are updates for permission purposes.
        ChangeType::UpdateCells => WriteAction::Update,
        ChangeType::CreateSpreadsheet => WriteAction::CreateSpreadsheet,
        ChangeType::CreateSheet => WriteAction::CreateSheet,
        ChangeType::DeleteSheet => WriteAction::DeleteSheet,
    }
}

#[cfg(test)]
#[path = "changes_tests.rs"]
mod tests;
