//! SQL implementations behind the Tauri commands (see docs/ipc.md).
//! Every function takes a plain `&Connection` so unit tests can run against
//! isolated temp-file databases without any Tauri state.

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::db::now_iso;
use crate::models::{
    AppStatus, AuditEvent, DataSource, PendingChange, PermissionRuleRow, SavePermissionRule,
    TablePage, TableRecord, TableRef, TableSchema,
};

/// A heartbeat row younger than this means the MCP sidecar is running.
const HEARTBEAT_FRESH_SECS: i64 = 30;
const DEFAULT_PAGE_LIMIT: i64 = 100;
const MIN_PAGE_LIMIT: i64 = 1;
const MAX_PAGE_LIMIT: i64 = 500;
const CHANGES_LIST_LIMIT: i64 = 200;
/// Must match `ConfirmationAction` in `@sheet-port/shared`.
const ALLOWED_CONFIRMATION_ACTIONS: [&str; 5] = [
    "append",
    "update",
    "delete",
    "bulk_update",
    "formula_change",
];
const CHANGE_STATUSES: [&str; 4] = ["pending", "approved", "committed", "rejected"];

#[derive(Debug, Clone, Copy)]
pub enum ChangeDecision {
    Approve,
    Reject,
}

impl ChangeDecision {
    fn new_status(self) -> &'static str {
        match self {
            Self::Approve => "approved",
            Self::Reject => "rejected",
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

fn db_error(context: &str, error: rusqlite::Error) -> String {
    format!("{context}: {error}")
}

fn parse_json(raw: &str, context: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(raw).map_err(|error| format!("{context} is not valid JSON: {error}"))
}

fn clamp_page(limit: Option<i64>, offset: Option<i64>) -> (i64, i64) {
    let limit = limit
        .unwrap_or(DEFAULT_PAGE_LIMIT)
        .clamp(MIN_PAGE_LIMIT, MAX_PAGE_LIMIT);
    let offset = offset.unwrap_or(0).max(0);
    (limit, offset)
}

// ---------------------------------------------------------------------------
// App status
// ---------------------------------------------------------------------------

pub fn app_status(
    conn: &Connection,
    app_version: String,
    db_path: String,
) -> Result<AppStatus, String> {
    let heartbeat: Option<(i64, String)> = conn
        .query_row(
            "SELECT pid, last_seen FROM mcp_heartbeat ORDER BY last_seen DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| db_error("Could not read MCP heartbeat", error))?;

    let pending_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pending_changes WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| db_error("Could not count pending changes", error))?;

    let (mcp_pid, mcp_last_seen) = match &heartbeat {
        Some((pid, last_seen)) => (Some(*pid), Some(last_seen.clone())),
        None => (None, None),
    };
    let mcp_running = heartbeat
        .as_ref()
        .is_some_and(|(_, last_seen)| is_heartbeat_fresh(last_seen));

    Ok(AppStatus {
        app_version,
        db_path,
        mcp_running,
        mcp_pid,
        mcp_last_seen,
        pending_count,
    })
}

fn is_heartbeat_fresh(last_seen: &str) -> bool {
    match chrono::DateTime::parse_from_rfc3339(last_seen) {
        Ok(seen) => {
            let age = chrono::Utc::now().signed_duration_since(seen.with_timezone(&chrono::Utc));
            age.num_seconds() <= HEARTBEAT_FRESH_SECS && age.num_seconds() >= -HEARTBEAT_FRESH_SECS
        }
        Err(error) => {
            eprintln!("[sheet-port] unparseable mcp_heartbeat.last_seen '{last_seen}': {error}");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Sources and tables
// ---------------------------------------------------------------------------

pub fn list_sources(conn: &Connection) -> Result<Vec<DataSource>, String> {
    let mut stmt = conn
        .prepare("SELECT id, kind, name, status FROM sources ORDER BY id")
        .map_err(|error| db_error("Could not list sources", error))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DataSource {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                status: row.get(3)?,
            })
        })
        .map_err(|error| db_error("Could not list sources", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list sources", error))
}

pub fn list_tables(conn: &Connection, source_id: &str) -> Result<Vec<TableRef>, String> {
    let mut stmt = conn
        .prepare("SELECT source_id, table_id, name FROM mock_tables WHERE source_id = ?1 ORDER BY table_id")
        .map_err(|error| db_error("Could not list tables", error))?;
    let rows = stmt
        .query_map([source_id], |row| {
            Ok(TableRef {
                source_id: row.get(0)?,
                table_id: row.get(1)?,
                name: row.get(2)?,
            })
        })
        .map_err(|error| db_error("Could not list tables", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list tables", error))
}

pub fn describe_table(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
) -> Result<TableSchema, String> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT name, fields FROM mock_tables WHERE source_id = ?1 AND table_id = ?2",
            params![source_id, table_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| db_error("Could not describe table", error))?;
    let (name, fields_raw) = row.ok_or_else(|| format!("Unknown table {source_id}/{table_id}"))?;
    Ok(TableSchema {
        source_id: source_id.to_string(),
        table_id: table_id.to_string(),
        name,
        fields: parse_json(&fields_raw, "Table field schema")?,
    })
}

pub fn read_table(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<TablePage, String> {
    let (limit, offset) = clamp_page(limit, offset);
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM mock_records WHERE source_id = ?1 AND table_id = ?2",
            params![source_id, table_id],
            |row| row.get(0),
        )
        .map_err(|error| db_error("Could not count records", error))?;

    let mut stmt = conn
        .prepare(
            "SELECT record_id, fields FROM mock_records
             WHERE source_id = ?1 AND table_id = ?2
             ORDER BY position LIMIT ?3 OFFSET ?4",
        )
        .map_err(|error| db_error("Could not read records", error))?;
    let raw_rows = stmt
        .query_map(params![source_id, table_id, limit, offset], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| db_error("Could not read records", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not read records", error))?;

    let records = raw_rows
        .into_iter()
        .map(|(id, fields_raw)| {
            Ok(TableRecord {
                fields: parse_json(&fields_raw, &format!("Record {id} fields"))?,
                id,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(TablePage { records, total })
}

// ---------------------------------------------------------------------------
// Permission rules
// ---------------------------------------------------------------------------

fn map_permission_rule(row: &Row<'_>) -> rusqlite::Result<(PermissionRuleRow, String)> {
    let raw_confirmation: String = row.get(6)?;
    Ok((
        PermissionRuleRow {
            id: row.get(0)?,
            source_id: row.get(1)?,
            table_id: row.get(2)?,
            read: row.get(3)?,
            write: row.get(4)?,
            delete_records: row.get(5)?,
            require_confirmation_for: Vec::new(),
            updated_at: row.get(7)?,
        },
        raw_confirmation,
    ))
}

fn finish_permission_rule(
    (mut rule, raw_confirmation): (PermissionRuleRow, String),
) -> Result<PermissionRuleRow, String> {
    rule.require_confirmation_for = serde_json::from_str(&raw_confirmation).map_err(|error| {
        format!(
            "Permission rule {} has invalid require_confirmation JSON: {error}",
            rule.id
        )
    })?;
    Ok(rule)
}

const PERMISSION_RULE_COLUMNS: &str =
    "id, source_id, table_id, can_read, can_write, can_delete, require_confirmation, updated_at";

pub fn list_permission_rules(conn: &Connection) -> Result<Vec<PermissionRuleRow>, String> {
    let sql = format!(
        "SELECT {PERMISSION_RULE_COLUMNS} FROM permission_rules ORDER BY source_id, table_id"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|error| db_error("Could not list permission rules", error))?;
    let rows = stmt
        .query_map([], map_permission_rule)
        .map_err(|error| db_error("Could not list permission rules", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list permission rules", error))?;
    rows.into_iter().map(finish_permission_rule).collect()
}

fn get_permission_rule(conn: &Connection, id: i64) -> Result<PermissionRuleRow, String> {
    let sql = format!("SELECT {PERMISSION_RULE_COLUMNS} FROM permission_rules WHERE id = ?1");
    let row = conn
        .query_row(&sql, [id], map_permission_rule)
        .optional()
        .map_err(|error| db_error("Could not load permission rule", error))?
        .ok_or_else(|| format!("Permission rule {id} was not found"))?;
    finish_permission_rule(row)
}

fn validate_confirmation_actions(actions: &[String]) -> Result<(), String> {
    for action in actions {
        if !ALLOWED_CONFIRMATION_ACTIONS.contains(&action.as_str()) {
            return Err(format!(
                "Invalid confirmation action '{action}'. Allowed: {}",
                ALLOWED_CONFIRMATION_ACTIONS.join(", ")
            ));
        }
    }
    Ok(())
}

pub fn save_permission_rule(
    conn: &Connection,
    rule: &SavePermissionRule,
) -> Result<PermissionRuleRow, String> {
    validate_confirmation_actions(&rule.require_confirmation_for)?;
    let confirmation_json = serde_json::to_string(&rule.require_confirmation_for)
        .map_err(|error| format!("Could not encode confirmation actions: {error}"))?;
    let rule_id = upsert_permission_rule(conn, rule, &confirmation_json)?;

    let snapshot = serde_json::to_value(rule)
        .map_err(|error| format!("Could not encode permission rule snapshot: {error}"))?;
    insert_audit_event(
        conn,
        "user",
        "permission_rule_saved",
        Some(&rule.source_id),
        rule.table_id.as_deref(),
        Some(&snapshot),
    )?;
    get_permission_rule(conn, rule_id)
}

fn upsert_permission_rule(
    conn: &Connection,
    rule: &SavePermissionRule,
    confirmation_json: &str,
) -> Result<i64, String> {
    // SQLite's UNIQUE(source_id, table_id) treats NULL table_id rows as
    // distinct, so ON CONFLICT would not fire for source-wide rules. Resolve
    // the target row explicitly instead; the connection is behind one Mutex,
    // so this select-then-write pair cannot race.
    let target_id = match rule.id {
        Some(id) => Some(id),
        None => conn
            .query_row(
                "SELECT id FROM permission_rules WHERE source_id = ?1 AND table_id IS ?2",
                params![rule.source_id, rule.table_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| db_error("Could not look up permission rule", error))?,
    };
    match target_id {
        Some(id) => update_permission_rule_row(conn, id, rule, confirmation_json),
        None => insert_permission_rule_row(conn, rule, confirmation_json),
    }
}

fn update_permission_rule_row(
    conn: &Connection,
    id: i64,
    rule: &SavePermissionRule,
    confirmation_json: &str,
) -> Result<i64, String> {
    let updated = conn
        .execute(
            "UPDATE permission_rules
             SET source_id = ?1, table_id = ?2, can_read = ?3, can_write = ?4,
                 can_delete = ?5, require_confirmation = ?6, updated_at = ?7
             WHERE id = ?8",
            params![
                rule.source_id,
                rule.table_id,
                rule.read,
                rule.write,
                rule.delete_records,
                confirmation_json,
                now_iso(),
                id
            ],
        )
        .map_err(|error| db_error("Could not update permission rule", error))?;
    if updated == 0 {
        return Err(format!("Permission rule {id} was not found"));
    }
    Ok(id)
}

fn insert_permission_rule_row(
    conn: &Connection,
    rule: &SavePermissionRule,
    confirmation_json: &str,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO permission_rules
             (source_id, table_id, can_read, can_write, can_delete,
              require_confirmation, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            rule.source_id,
            rule.table_id,
            rule.read,
            rule.write,
            rule.delete_records,
            confirmation_json,
            now_iso()
        ],
    )
    .map_err(|error| db_error("Could not insert permission rule", error))?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_permission_rule(conn: &Connection, id: i64) -> Result<(), String> {
    let existing: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT source_id, table_id FROM permission_rules WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| db_error("Could not look up permission rule", error))?;
    let (source_id, table_id) =
        existing.ok_or_else(|| format!("Permission rule {id} was not found"))?;

    conn.execute("DELETE FROM permission_rules WHERE id = ?1", [id])
        .map_err(|error| db_error("Could not delete permission rule", error))?;
    insert_audit_event(
        conn,
        "user",
        "permission_rule_deleted",
        Some(&source_id),
        table_id.as_deref(),
        Some(&serde_json::json!({ "ruleId": id })),
    )
}

// ---------------------------------------------------------------------------
// Pending changes
// ---------------------------------------------------------------------------

// The internal `payload` column is intentionally absent: it must never reach
// the frontend or agents through the desktop IPC.
const PENDING_CHANGE_COLUMNS: &str = "id, source_id, table_id, change_type, created_at, status, \
     requires_confirmation, diff, decided_at, decided_by, committed_at";

fn map_pending_change(row: &Row<'_>) -> rusqlite::Result<(PendingChange, String)> {
    let raw_diff: String = row.get(7)?;
    Ok((
        PendingChange {
            id: row.get(0)?,
            source_id: row.get(1)?,
            table_id: row.get(2)?,
            change_type: row.get(3)?,
            created_at: row.get(4)?,
            status: row.get(5)?,
            requires_confirmation: row.get(6)?,
            diff: serde_json::Value::Null,
            decided_at: row.get(8)?,
            decided_by: row.get(9)?,
            committed_at: row.get(10)?,
        },
        raw_diff,
    ))
}

fn finish_pending_change(
    (mut change, raw_diff): (PendingChange, String),
) -> Result<PendingChange, String> {
    change.diff = parse_json(&raw_diff, &format!("Diff of change {}", change.id))?;
    Ok(change)
}

pub fn list_changes(conn: &Connection, status: Option<&str>) -> Result<Vec<PendingChange>, String> {
    if let Some(status) = status {
        if !CHANGE_STATUSES.contains(&status) {
            return Err(format!(
                "Unknown change status '{status}'. Allowed: {}",
                CHANGE_STATUSES.join(", ")
            ));
        }
    }
    let sql = format!(
        "SELECT {PENDING_CHANGE_COLUMNS} FROM pending_changes
         WHERE (?1 IS NULL OR status = ?1)
         ORDER BY created_at DESC LIMIT {CHANGES_LIST_LIMIT}"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|error| db_error("Could not list changes", error))?;
    let rows = stmt
        .query_map(params![status], map_pending_change)
        .map_err(|error| db_error("Could not list changes", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list changes", error))?;
    rows.into_iter().map(finish_pending_change).collect()
}

fn get_change(conn: &Connection, change_id: &str) -> Result<PendingChange, String> {
    let sql = format!("SELECT {PENDING_CHANGE_COLUMNS} FROM pending_changes WHERE id = ?1");
    let row = conn
        .query_row(&sql, [change_id], map_pending_change)
        .optional()
        .map_err(|error| db_error("Could not load change", error))?
        .ok_or_else(|| format!("Change {change_id} was not found"))?;
    finish_pending_change(row)
}

pub fn decide_change(
    conn: &Connection,
    change_id: &str,
    decision: ChangeDecision,
) -> Result<PendingChange, String> {
    let updated = conn
        .execute(
            "UPDATE pending_changes
             SET status = ?1, decided_at = ?2, decided_by = 'user'
             WHERE id = ?3 AND status = 'pending'",
            params![decision.new_status(), now_iso(), change_id],
        )
        .map_err(|error| db_error("Could not update change", error))?;
    if updated == 0 {
        let current: Option<String> = conn
            .query_row(
                "SELECT status FROM pending_changes WHERE id = ?1",
                [change_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| db_error("Could not read change status", error))?;
        return Err(match current {
            Some(status) => format!(
                "Change {change_id} cannot be {}: its status is '{status}', not 'pending'",
                decision.verb()
            ),
            None => format!("Change {change_id} was not found"),
        });
    }

    let change = get_change(conn, change_id)?;
    insert_audit_event(
        conn,
        "user",
        decision.audit_action(),
        Some(&change.source_id),
        Some(&change.table_id),
        Some(&serde_json::json!({ "changeId": change_id })),
    )?;
    Ok(change)
}

// ---------------------------------------------------------------------------
// Audit events
// ---------------------------------------------------------------------------

pub fn insert_audit_event(
    conn: &Connection,
    actor: &str,
    action: &str,
    source_id: Option<&str>,
    table_id: Option<&str>,
    metadata: Option<&serde_json::Value>,
) -> Result<(), String> {
    let metadata_json = metadata
        .map(|value| {
            serde_json::to_string(value)
                .map_err(|error| format!("Could not encode audit metadata: {error}"))
        })
        .transpose()?;
    conn.execute(
        "INSERT INTO audit_events (id, timestamp, actor, action, source_id, table_id, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            uuid::Uuid::new_v4().to_string(),
            now_iso(),
            actor,
            action,
            source_id,
            table_id,
            metadata_json
        ],
    )
    .map_err(|error| db_error("Could not write audit event", error))?;
    Ok(())
}

pub fn list_audit_events(
    conn: &Connection,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AuditEvent>, String> {
    let (limit, offset) = clamp_page(limit, offset);
    let mut stmt = conn
        .prepare(
            "SELECT id, timestamp, actor, action, source_id, table_id, metadata
             FROM audit_events ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
        )
        .map_err(|error| db_error("Could not list audit events", error))?;
    let rows = stmt
        .query_map(params![limit, offset], |row| {
            Ok((
                AuditEvent {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    actor: row.get(2)?,
                    action: row.get(3)?,
                    source_id: row.get(4)?,
                    table_id: row.get(5)?,
                    metadata: None,
                },
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|error| db_error("Could not list audit events", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list audit events", error))?;

    rows.into_iter()
        .map(|(mut event, raw_metadata)| {
            event.metadata = raw_metadata
                .map(|raw| parse_json(&raw, &format!("Metadata of audit event {}", event.id)))
                .transpose()?;
            Ok(event)
        })
        .collect()
}

#[cfg(test)]
#[path = "queries_tests.rs"]
mod tests;
