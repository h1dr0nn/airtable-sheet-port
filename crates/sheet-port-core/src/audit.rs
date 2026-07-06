//! Append-only audit log. Every agent tool call and every desktop decision
//! records an event; the list is served newest first.

use rusqlite::{params, Connection};
use serde_json::Value;

use crate::constants::{AUDIT_LIMIT_DEFAULT, AUDIT_LIMIT_MAX};
use crate::db::now_iso;
use crate::error::{db_error, parse_json, CoreError};
use crate::types::{AuditActor, AuditEvent};

const AUDIT_LIMIT_MIN: i64 = 1;

/// Inserts a new event with a generated `evt_` id and the current UTC
/// timestamp, returning the stored event.
pub fn record(
    conn: &Connection,
    actor: AuditActor,
    action: &str,
    source_id: Option<&str>,
    table_id: Option<&str>,
    metadata: Option<&Value>,
) -> Result<AuditEvent, CoreError> {
    let event = AuditEvent {
        id: format!("evt_{}", uuid::Uuid::new_v4()),
        timestamp: now_iso(),
        actor,
        action: action.to_string(),
        source_id: source_id.map(str::to_string),
        table_id: table_id.map(str::to_string),
        metadata: metadata.cloned(),
    };
    let metadata_json = event
        .metadata
        .as_ref()
        .map(|value| {
            serde_json::to_string(value).map_err(|error| {
                CoreError::Storage(format!("Could not encode audit metadata: {error}"))
            })
        })
        .transpose()?;
    conn.execute(
        "INSERT INTO audit_events (id, timestamp, actor, action, source_id, table_id, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            event.id,
            event.timestamp,
            event.actor.as_str(),
            event.action,
            event.source_id,
            event.table_id,
            metadata_json
        ],
    )
    .map_err(|error| db_error("Could not write audit event", error))?;
    Ok(event)
}

/// Newest first. Limit defaults to 100 and clamps to 1..=500; offset floors
/// at 0. The rowid tiebreaker keeps same-millisecond events in insertion
/// order (newest insert first).
pub fn list(
    conn: &Connection,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AuditEvent>, CoreError> {
    let limit = limit
        .unwrap_or(AUDIT_LIMIT_DEFAULT)
        .clamp(AUDIT_LIMIT_MIN, AUDIT_LIMIT_MAX);
    let offset = offset.unwrap_or(0).max(0);

    let mut stmt = conn
        .prepare(
            "SELECT id, timestamp, actor, action, source_id, table_id, metadata
             FROM audit_events ORDER BY timestamp DESC, rowid DESC LIMIT ?1 OFFSET ?2",
        )
        .map_err(|error| db_error("Could not list audit events", error))?;
    let rows = stmt
        .query_map(params![limit, offset], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|error| db_error("Could not list audit events", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list audit events", error))?;

    rows.into_iter()
        .map(
            |(id, timestamp, actor, action, source_id, table_id, raw_metadata)| {
                let actor = AuditActor::from_db(&actor).ok_or_else(|| {
                    CoreError::Storage(format!("Audit event {id} has unknown actor '{actor}'"))
                })?;
                let metadata = raw_metadata
                    .map(|raw| parse_json(&raw, &format!("Metadata of audit event {id}")))
                    .transpose()?;
                Ok(AuditEvent {
                    id,
                    timestamp,
                    actor,
                    action,
                    source_id,
                    table_id,
                    metadata,
                })
            },
        )
        .collect()
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
