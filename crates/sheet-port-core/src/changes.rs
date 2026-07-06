//! Pending-change lifecycle: preview -> (approve | reject) -> commit.
//! Enforcement per docs/ipc.md "Confirmation enforcement": the desktop app
//! approves/rejects rows in the shared DB and the broker reads fresh state at
//! commit time, so decisions apply across processes without direct IPC.
//! Every error message here is part of the observable contract.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audit;
use crate::connectors::ConnectorRegistry;
use crate::constants::{
    BULK_UPDATE_THRESHOLD, CHANGE_LIST_LIMIT, META_AUTO_APPROVE_WRITES, META_FLAG_ON,
};
use crate::db::{get_meta, now_iso};
use crate::error::{db_error, parse_json, CoreError};
use crate::permissions;
use crate::types::{
    AuditActor, ChangeDecider, ChangeStatus, ChangeType, JsonMap, PendingChange, ReadOptions,
    RecordPatch, TableRecord, WriteAction,
};

/// Internal write payload; persisted alongside a change but never returned to
/// agents or the desktop frontend. JSON shape matches the TypeScript
/// reference: {"type":"append","records":[...]}, {"type":"update",
/// "patches":[...]}, {"type":"delete","recordIds":[...]}.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ChangePayload {
    Append {
        records: Vec<JsonMap>,
    },
    Update {
        patches: Vec<RecordPatch>,
    },
    #[serde(rename_all = "camelCase")]
    Delete {
        record_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitOutcome {
    pub change: PendingChange,
    pub records: Vec<TableRecord>,
}

// The internal `payload` column is intentionally absent: it must never reach
// agents or the desktop frontend.
const CHANGE_COLUMNS: &str = "id, source_id, table_id, change_type, created_at, status, \
     requires_confirmation, diff, decided_at, decided_by, committed_at";

// ---------------------------------------------------------------------------
// Previews
// ---------------------------------------------------------------------------

pub fn create_append_change(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    records: Vec<JsonMap>,
    requires_confirmation: bool,
) -> Result<PendingChange, CoreError> {
    let diff = serde_json::json!({ "after": records });
    insert_change(
        conn,
        source_id,
        table_id,
        ChangeType::Append,
        &ChangePayload::Append { records },
        diff,
        requires_confirmation,
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
    requires_confirmation: bool,
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
        requires_confirmation,
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
    requires_confirmation: bool,
) -> Result<PendingChange, CoreError> {
    let change = PendingChange {
        id: format!("chg_{}", uuid::Uuid::new_v4()),
        source_id: source_id.to_string(),
        table_id: table_id.to_string(),
        change_type,
        created_at: now_iso(),
        status: ChangeStatus::Pending,
        requires_confirmation,
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
            change.requires_confirmation,
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
        requires_confirmation: row.get(6)?,
        diff: row.get(7)?,
        decided_at: row.get(8)?,
        decided_by: row.get(9)?,
        committed_at: row.get(10)?,
    })
}

struct RawChange {
    id: String,
    source_id: String,
    table_id: String,
    change_type: String,
    created_at: String,
    status: String,
    requires_confirmation: bool,
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
            requires_confirmation: self.requires_confirmation,
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

// ---------------------------------------------------------------------------
// Desktop decisions (approve / reject)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum ChangeDecision {
    Approve,
    Reject,
}

impl ChangeDecision {
    fn new_status(self) -> ChangeStatus {
        match self {
            Self::Approve => ChangeStatus::Approved,
            Self::Reject => ChangeStatus::Rejected,
        }
    }

    fn audit_action(self) -> &'static str {
        match self {
            Self::Approve => "change_approved",
            Self::Reject => "change_rejected",
        }
    }

    fn verb(self) -> &'static str {
        match self {
            Self::Approve => "approved",
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
    // The confirmation gate normally blocks a requires_confirmation change that
    // the user has not approved. The auto-approve opt-in (read fresh here, never
    // cached) bypasses it: the change is treated as policy-approved instead.
    // Default off keeps the human-in-the-loop guarantee (see docs/security.md).
    if change.requires_confirmation
        && change.status != ChangeStatus::Approved
        && !auto_approve_enabled(conn)?
    {
        return Err(CoreError::Conflict(format!(
            "Change {change_id} requires user approval in the Airtable - Sheet Port desktop app before commit"
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

    if change.status == ChangeStatus::Pending {
        // Reached when requires_confirmation is false, or when it is true but the
        // auto-approve opt-in bypassed the gate above: either way policy
        // auto-approves before the write.
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

    let records = execute(conn, registry, &change, &payload)?;
    if !mark_committed(conn, change_id)? {
        return Err(CoreError::Conflict(format!(
            "Change {change_id} could not be marked committed (state changed concurrently)"
        )));
    }
    let committed = get_change(conn, change_id)?.ok_or_else(|| {
        CoreError::Storage(format!("Change {change_id} disappeared after commit"))
    })?;
    Ok(CommitOutcome {
        change: committed,
        records,
    })
}

fn execute(
    conn: &Connection,
    registry: &ConnectorRegistry,
    change: &PendingChange,
    payload: &ChangePayload,
) -> Result<Vec<TableRecord>, CoreError> {
    match payload {
        ChangePayload::Append { records } => {
            registry.append_records(conn, &change.source_id, &change.table_id, records)
        }
        ChangePayload::Update { patches } => {
            registry.update_records(conn, &change.source_id, &change.table_id, patches)
        }
        ChangePayload::Delete { .. } => Err(CoreError::Unsupported(
            "Delete changes are not implemented in the MVP".to_string(),
        )),
    }
}

/// Reads the auto-approve-writes opt-in fresh from `meta`. On means a
/// requires_confirmation change may commit without a desktop approval; the
/// setting is intentionally read at commit time so a desktop toggle applies
/// across processes without any direct IPC.
fn auto_approve_enabled(conn: &Connection) -> Result<bool, CoreError> {
    Ok(get_meta(conn, META_AUTO_APPROVE_WRITES)?.as_deref() == Some(META_FLAG_ON))
}

/// Re-derives the evaluated action so commit re-checks the same policy the
/// preview used: large updates escalate to bulk_update.
fn commit_action(change_type: ChangeType, payload: &ChangePayload) -> WriteAction {
    if let ChangePayload::Update { patches } = payload {
        if patches.len() > BULK_UPDATE_THRESHOLD {
            return WriteAction::BulkUpdate;
        }
    }
    match change_type {
        ChangeType::Append => WriteAction::Append,
        ChangeType::Update => WriteAction::Update,
        ChangeType::Delete => WriteAction::Delete,
    }
}

#[cfg(test)]
#[path = "changes_tests.rs"]
mod tests;
