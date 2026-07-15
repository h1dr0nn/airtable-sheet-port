//! The broker tools (docs/mcp-tools.md): permission checks in a fixed order,
//! an audit event per call, and pretty-printed JSON output. Each function
//! returns the JSON text of the tool result; errors bubble as `CoreError` and
//! the transport layer turns them into MCP tool errors.

use serde::Serialize;
use serde_json::json;
use sheet_port_core::constants::BULK_UPDATE_THRESHOLD;
use sheet_port_core::types::{
    AuditActor, AuditEvent, ChangeType, DataSource, PendingChange, ReadOptions, RecordPatch,
    TableRecord, TableRef, TableSchema, TableStyle, WriteAction,
};
use sheet_port_core::{audit, changes, permissions, CoreError};

use crate::args::{
    AppendRecordsArgs, CommitChangeArgs, FindRecordsArgs, FormatTableArgs, GetAuditLogArgs,
    ListTablesArgs, PreviewUpdateArgs, ReadTableArgs, SourceTableArgs,
};
use crate::state::BrokerState;

#[derive(Serialize)]
struct SourcesOutput {
    sources: Vec<DataSource>,
}

#[derive(Serialize)]
struct TablesOutput {
    tables: Vec<TableRef>,
}

#[derive(Serialize)]
struct SchemaOutput {
    schema: TableSchema,
}

#[derive(Serialize)]
struct RecordsOutput {
    records: Vec<TableRecord>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewOutput {
    change: PendingChange,
    requires_confirmation: bool,
}

/// Response for a batch `commit_change` (the plural `changeIds` form): one
/// outcome per committed change, in the order requested.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitBatchOutput {
    committed: Vec<changes::CommitOutcome>,
}

#[derive(Serialize)]
struct EventsOutput {
    events: Vec<AuditEvent>,
}

#[derive(Serialize)]
struct StyleOutput {
    style: TableStyle,
}

/// JSON.stringify(data, null, 2) equivalent: 2-space pretty printing.
fn pretty<T: Serialize>(value: &T) -> Result<String, CoreError> {
    serde_json::to_string_pretty(value)
        .map_err(|error| CoreError::Storage(format!("Could not encode tool result: {error}")))
}

pub fn list_sources(state: &BrokerState) -> Result<String, CoreError> {
    state.with_conn(|conn, registry| {
        let sources = registry.list_sources(conn)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "list_sources",
            None,
            None,
            Some(&json!({ "count": sources.len() })),
        )?;
        pretty(&SourcesOutput { sources })
    })
}

pub fn list_tables(state: &BrokerState, args: &ListTablesArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        permissions::assert_can_read(conn, &args.source_id, None)?;
        let tables = registry.list_tables(conn, &args.source_id)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "list_tables",
            Some(&args.source_id),
            None,
            Some(&json!({ "count": tables.len() })),
        )?;
        pretty(&TablesOutput { tables })
    })
}

pub fn describe_table(state: &BrokerState, args: &SourceTableArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        permissions::assert_can_read(conn, &args.source_id, Some(&args.table_id))?;
        let schema = registry.describe_table(conn, &args.source_id, &args.table_id)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "describe_table",
            Some(&args.source_id),
            Some(&args.table_id),
            None,
        )?;
        pretty(&SchemaOutput { schema })
    })
}

pub fn read_table(state: &BrokerState, args: &ReadTableArgs) -> Result<String, CoreError> {
    let (limit, offset) = args.validate()?;
    state.with_conn(|conn, registry| {
        permissions::assert_can_read(conn, &args.source_id, Some(&args.table_id))?;
        let options = ReadOptions {
            limit: Some(limit),
            offset: Some(offset),
        };
        let records = registry.read_table(conn, &args.source_id, &args.table_id, options)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "read_table",
            Some(&args.source_id),
            Some(&args.table_id),
            Some(&json!({ "limit": limit, "offset": offset, "count": records.len() })),
        )?;
        pretty(&RecordsOutput { records })
    })
}

pub fn read_formulas(state: &BrokerState, args: &ReadTableArgs) -> Result<String, CoreError> {
    let (limit, offset) = args.validate()?;
    state.with_conn(|conn, registry| {
        permissions::assert_can_read(conn, &args.source_id, Some(&args.table_id))?;
        let options = ReadOptions {
            limit: Some(limit),
            offset: Some(offset),
        };
        let records = registry.read_formulas(conn, &args.source_id, &args.table_id, options)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "read_formulas",
            Some(&args.source_id),
            Some(&args.table_id),
            Some(&json!({ "limit": limit, "offset": offset, "count": records.len() })),
        )?;
        pretty(&RecordsOutput { records })
    })
}

pub fn find_records(state: &BrokerState, args: &FindRecordsArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        permissions::assert_can_read(conn, &args.source_id, Some(&args.table_id))?;
        let records = registry.find_records(conn, &args.source_id, &args.table_id, &args.query)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "find_records",
            Some(&args.source_id),
            Some(&args.table_id),
            Some(&json!({ "query": args.query, "count": records.len() })),
        )?;
        pretty(&RecordsOutput { records })
    })
}

pub fn preview_update_records(
    state: &BrokerState,
    args: PreviewUpdateArgs,
) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        // Read permission is checked first: the diff exposes current record
        // values (parity with the TypeScript tool).
        permissions::assert_can_read(conn, &args.source_id, Some(&args.table_id))?;
        let action = if args.patches.len() > BULK_UPDATE_THRESHOLD {
            WriteAction::BulkUpdate
        } else {
            WriteAction::Update
        };
        let requires_confirmation =
            permissions::assert_can_write(conn, &args.source_id, &args.table_id, action)?;
        let patches: Vec<RecordPatch> = args
            .patches
            .into_iter()
            .map(|patch| RecordPatch {
                record_id: patch.record_id,
                fields: patch.fields,
            })
            .collect();
        let patch_count = patches.len();
        let change = changes::create_update_change(
            conn,
            registry,
            &args.source_id,
            &args.table_id,
            patches,
            requires_confirmation,
        )?;
        audit::record(
            conn,
            AuditActor::Agent,
            "preview_update_records",
            Some(&args.source_id),
            Some(&args.table_id),
            Some(&json!({
                "changeId": change.id,
                "patchCount": patch_count,
                "requiresConfirmation": requires_confirmation,
            })),
        )?;
        pretty(&PreviewOutput {
            change,
            requires_confirmation,
        })
    })
}

pub fn append_records(state: &BrokerState, args: AppendRecordsArgs) -> Result<String, CoreError> {
    let format = args.validate()?;
    state.with_conn(|conn, _registry| {
        let mut requires_confirmation = permissions::assert_can_write(
            conn,
            &args.source_id,
            &args.table_id,
            WriteAction::Append,
        )?;
        // A bundled format plan is also a write; require confirmation if either
        // the append or the format action asks for it.
        if format.is_some() {
            let format_requires = permissions::assert_can_write(
                conn,
                &args.source_id,
                &args.table_id,
                WriteAction::Format,
            )?;
            requires_confirmation = requires_confirmation || format_requires;
        }
        let record_count = args.records.len();
        let has_format = format.is_some();
        let change = changes::create_append_with_format(
            conn,
            &args.source_id,
            &args.table_id,
            args.records,
            format,
            requires_confirmation,
        )?;
        audit::record(
            conn,
            AuditActor::Agent,
            "append_records_preview",
            Some(&args.source_id),
            Some(&args.table_id),
            Some(&json!({
                "changeId": change.id,
                "recordCount": record_count,
                "hasFormat": has_format,
                "requiresConfirmation": requires_confirmation,
            })),
        )?;
        pretty(&PreviewOutput {
            change,
            requires_confirmation,
        })
    })
}

pub fn commit_change(state: &BrokerState, args: &CommitChangeArgs) -> Result<String, CoreError> {
    let change_ids = args.ids()?;
    state.with_conn(|conn, registry| {
        // Pre-check each change's write permission with the raw change type
        // (parity with the TypeScript tool); changes::commit re-checks with the
        // exact preview action, including the bulk_update escalation.
        for change_id in &change_ids {
            let pending = changes::get_change(conn, change_id)?
                .ok_or_else(|| CoreError::NotFound(format!("Unknown change {change_id}")))?;
            permissions::assert_can_write(
                conn,
                &pending.source_id,
                &pending.table_id,
                write_action_for(pending.change_type),
            )?;
        }

        if args.is_batch() {
            let outcomes = changes::commit_many(conn, registry, &change_ids)?;
            let record_count: usize = outcomes.iter().map(|outcome| outcome.records.len()).sum();
            audit::record(
                conn,
                AuditActor::Agent,
                "commit_change",
                None,
                None,
                Some(&json!({
                    "changeIds": change_ids,
                    "count": outcomes.len(),
                    "recordCount": record_count,
                })),
            )?;
            pretty(&CommitBatchOutput {
                committed: outcomes,
            })
        } else {
            let outcome = changes::commit(conn, registry, &change_ids[0])?;
            audit::record(
                conn,
                AuditActor::Agent,
                "commit_change",
                Some(&outcome.change.source_id),
                Some(&outcome.change.table_id),
                Some(&json!({
                    "changeId": change_ids[0],
                    "recordCount": outcome.records.len(),
                })),
            )?;
            pretty(&outcome)
        }
    })
}

pub fn get_table_style(state: &BrokerState, args: &SourceTableArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        permissions::assert_can_read(conn, &args.source_id, Some(&args.table_id))?;
        let style = registry.read_table_style(conn, &args.source_id, &args.table_id)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "get_table_style",
            Some(&args.source_id),
            Some(&args.table_id),
            None,
        )?;
        pretty(&StyleOutput { style })
    })
}

pub fn preview_format_table(
    state: &BrokerState,
    args: FormatTableArgs,
) -> Result<String, CoreError> {
    let plan = args.validate()?;
    state.with_conn(|conn, _registry| {
        let requires_confirmation = permissions::assert_can_write(
            conn,
            &args.source_id,
            &args.table_id,
            WriteAction::Format,
        )?;
        let format_count = plan.formats.len();
        let change = changes::create_format_change(
            conn,
            &args.source_id,
            &args.table_id,
            plan,
            requires_confirmation,
        )?;
        audit::record(
            conn,
            AuditActor::Agent,
            "preview_format_table",
            Some(&args.source_id),
            Some(&args.table_id),
            Some(&json!({
                "changeId": change.id,
                "formatCount": format_count,
                "requiresConfirmation": requires_confirmation,
            })),
        )?;
        pretty(&PreviewOutput {
            change,
            requires_confirmation,
        })
    })
}

pub fn get_audit_log(state: &BrokerState, args: &GetAuditLogArgs) -> Result<String, CoreError> {
    let limit = args.validate()?;
    state.with_conn(|conn, _registry| {
        // Recorded before listing so the call shows up in its own output
        // (parity with the TypeScript tool).
        audit::record(
            conn,
            AuditActor::Agent,
            "get_audit_log",
            None,
            None,
            Some(&json!({ "limit": limit })),
        )?;
        let events = audit::list(conn, Some(limit), None)?;
        pretty(&EventsOutput { events })
    })
}

fn write_action_for(change_type: ChangeType) -> WriteAction {
    match change_type {
        ChangeType::Append => WriteAction::Append,
        ChangeType::Update => WriteAction::Update,
        ChangeType::Delete => WriteAction::Delete,
        ChangeType::Format => WriteAction::Format,
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
