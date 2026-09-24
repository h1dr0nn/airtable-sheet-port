//! Permission rules and their evaluation. Rules are read fresh from the
//! database on every call (no caching) so edits made in the desktop app apply
//! to the MCP server immediately.

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::audit;
use crate::db::now_iso;
use crate::error::{db_error, CoreError};
use crate::types::{AuditActor, PermissionRuleRow, SavePermissionRule, WriteAction};

/// The legacy `require_confirmation` column is no longer read; saved rules
/// always store an empty JSON list there.
const RULE_COLUMNS: &str = "id, source_id, table_id, can_read, can_write, can_delete, updated_at";

/// Value persisted into the legacy `require_confirmation` column.
const EMPTY_CONFIRMATION_JSON: &str = "[]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteEvaluation {
    pub allowed: bool,
    pub reason: Option<String>,
}

fn map_rule(row: &Row<'_>) -> rusqlite::Result<PermissionRuleRow> {
    Ok(PermissionRuleRow {
        id: row.get(0)?,
        source_id: row.get(1)?,
        table_id: row.get(2)?,
        read: row.get(3)?,
        write: row.get(4)?,
        delete_records: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

/// Resolves the effective rule with table-specific precedence: an exact
/// (source_id, table_id) rule wins over the source-wide (NULL table_id) rule.
pub fn find_rule(
    conn: &Connection,
    source_id: &str,
    table_id: Option<&str>,
) -> Result<Option<PermissionRuleRow>, CoreError> {
    match table_id {
        Some(table_id) => conn
            .query_row(
                &format!(
                    "SELECT {RULE_COLUMNS} FROM permission_rules
                     WHERE source_id = ?1 AND (table_id = ?2 OR table_id IS NULL)
                     ORDER BY (table_id IS NULL) ASC LIMIT 1"
                ),
                params![source_id, table_id],
                map_rule,
            )
            .optional(),
        None => conn
            .query_row(
                &format!(
                    "SELECT {RULE_COLUMNS} FROM permission_rules
                     WHERE source_id = ?1 AND table_id IS NULL LIMIT 1"
                ),
                params![source_id],
                map_rule,
            )
            .optional(),
    }
    .map_err(|error| db_error("Could not look up permission rule", error))
}

pub fn evaluate_write(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    action: WriteAction,
) -> Result<WriteEvaluation, CoreError> {
    // A source-level action (e.g. create_spreadsheet, which has no table yet)
    // carries an empty table_id and is evaluated against the source-wide rule.
    let scope = if table_id.is_empty() {
        source_id.to_string()
    } else {
        format!("{source_id}/{table_id}")
    };
    let lookup_table = (!table_id.is_empty()).then_some(table_id);
    let rule = find_rule(conn, source_id, lookup_table)?;
    let Some(rule) = rule.filter(|rule| rule.write) else {
        return Ok(WriteEvaluation {
            allowed: false,
            reason: Some(format!("Write access denied for {scope}")),
        });
    };
    if action.needs_delete_permission() && !rule.delete_records {
        return Ok(WriteEvaluation {
            allowed: false,
            reason: Some(format!("Delete access denied for {scope}")),
        });
    }
    Ok(WriteEvaluation {
        allowed: true,
        reason: None,
    })
}

pub fn assert_can_read(
    conn: &Connection,
    source_id: &str,
    table_id: Option<&str>,
) -> Result<(), CoreError> {
    let rule = find_rule(conn, source_id, table_id)?;
    if !rule.is_some_and(|rule| rule.read) {
        let scope = match table_id {
            Some(table_id) => format!("{source_id}/{table_id}"),
            None => source_id.to_string(),
        };
        return Err(CoreError::PermissionDenied(format!(
            "Read access denied for {scope}"
        )));
    }
    Ok(())
}

/// Fails with `PermissionDenied` unless the effective rule allows the write.
pub fn assert_can_write(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    action: WriteAction,
) -> Result<(), CoreError> {
    let evaluation = evaluate_write(conn, source_id, table_id, action)?;
    if !evaluation.allowed {
        let reason = evaluation
            .reason
            .unwrap_or_else(|| format!("Write access denied for {source_id}/{table_id}"));
        return Err(CoreError::PermissionDenied(reason));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Desktop-side rule management (docs/ipc.md)
// ---------------------------------------------------------------------------

pub fn list_rules(conn: &Connection) -> Result<Vec<PermissionRuleRow>, CoreError> {
    let sql = format!("SELECT {RULE_COLUMNS} FROM permission_rules ORDER BY source_id, table_id");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|error| db_error("Could not list permission rules", error))?;
    let rows = stmt
        .query_map([], map_rule)
        .map_err(|error| db_error("Could not list permission rules", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list permission rules", error))?;
    Ok(rows)
}

fn get_rule(conn: &Connection, id: i64) -> Result<PermissionRuleRow, CoreError> {
    let sql = format!("SELECT {RULE_COLUMNS} FROM permission_rules WHERE id = ?1");
    conn.query_row(&sql, [id], map_rule)
        .optional()
        .map_err(|error| db_error("Could not load permission rule", error))?
        .ok_or_else(|| CoreError::NotFound(format!("Permission rule {id} was not found")))
}

pub fn save_rule(
    conn: &Connection,
    rule: &SavePermissionRule,
) -> Result<PermissionRuleRow, CoreError> {
    let rule_id = upsert_rule(conn, rule)?;

    let snapshot = serde_json::to_value(rule).map_err(|error| {
        CoreError::Storage(format!(
            "Could not encode permission rule snapshot: {error}"
        ))
    })?;
    audit::record(
        conn,
        AuditActor::User,
        "permission_rule_saved",
        Some(&rule.source_id),
        rule.table_id.as_deref(),
        Some(&snapshot),
    )?;
    get_rule(conn, rule_id)
}

fn upsert_rule(conn: &Connection, rule: &SavePermissionRule) -> Result<i64, CoreError> {
    // SQLite's UNIQUE(source_id, table_id) treats NULL table_id rows as
    // distinct, so ON CONFLICT would not fire for source-wide rules. Resolve
    // the target row explicitly instead; callers hold the single shared
    // connection, so this select-then-write pair cannot race.
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
        Some(id) => update_rule_row(conn, id, rule),
        None => insert_rule_row(conn, rule),
    }
}

fn update_rule_row(
    conn: &Connection,
    id: i64,
    rule: &SavePermissionRule,
) -> Result<i64, CoreError> {
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
                EMPTY_CONFIRMATION_JSON,
                now_iso(),
                id
            ],
        )
        .map_err(|error| db_error("Could not update permission rule", error))?;
    if updated == 0 {
        return Err(CoreError::NotFound(format!(
            "Permission rule {id} was not found"
        )));
    }
    Ok(id)
}

fn insert_rule_row(conn: &Connection, rule: &SavePermissionRule) -> Result<i64, CoreError> {
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
            EMPTY_CONFIRMATION_JSON,
            now_iso()
        ],
    )
    .map_err(|error| db_error("Could not insert permission rule", error))?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_rule(conn: &Connection, id: i64) -> Result<(), CoreError> {
    let existing: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT source_id, table_id FROM permission_rules WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| db_error("Could not look up permission rule", error))?;
    let (source_id, table_id) = existing
        .ok_or_else(|| CoreError::NotFound(format!("Permission rule {id} was not found")))?;

    conn.execute("DELETE FROM permission_rules WHERE id = ?1", [id])
        .map_err(|error| db_error("Could not delete permission rule", error))?;
    audit::record(
        conn,
        AuditActor::User,
        "permission_rule_deleted",
        Some(&source_id),
        table_id.as_deref(),
        Some(&serde_json::json!({ "ruleId": id })),
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
