//! The broker tools (docs/mcp-tools.md): source resolution, permission checks
//! in a fixed order, an audit event per call, and pretty-printed JSON output.
//! Each function returns the JSON text of the tool result; errors bubble as
//! `CoreError` and the transport layer turns them into MCP tool errors.
//!
//! Every tool takes an optional `sourceId`. It is resolved first through
//! [`google::resolve_source`] (omitted = auto-routed to the bridge that can
//! open the spreadsheet); permission checks and audit events then use the
//! resolved id. Write tools stage a change and commit it in the same call
//! unless `dryRun` is set, returning the staged diff either way.

use serde::Serialize;
use serde_json::{json, Value};
use sheet_port_core::connectors::{parse_spreadsheet_id, ConnectorRegistry};
use sheet_port_core::constants::BULK_UPDATE_THRESHOLD;
use sheet_port_core::rusqlite::Connection;
use sheet_port_core::types::{
    AuditActor, AuditEvent, ChangeType, DataSource, GridRow, PendingChange, ReadOptions,
    RecordPatch, TableRecord, TableRef, TableSchema, TableStyle, WriteAction,
};
use sheet_port_core::{audit, changes, google, permissions, CoreError};

use crate::args::{
    AppendRecordsArgs, CommitChangeArgs, CreateSheetArgs, CreateSpreadsheetArgs, DeleteSheetArgs,
    FindRecordsArgs, FormatTableArgs, GetAuditLogArgs, ListTablesArgs, ReadCellsArgs,
    ReadTableArgs, SourceTableArgs, UpdateCellsArgs, UpdateRecordsArgs,
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

/// One tab of a spreadsheet as `list_sheets` reports it.
#[derive(Serialize)]
struct SheetOutput {
    gid: String,
    title: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SheetsOutput {
    spreadsheet_id: String,
    sheets: Vec<SheetOutput>,
}

#[derive(Serialize)]
struct SchemaOutput {
    schema: TableSchema,
}

#[derive(Serialize)]
struct RecordsOutput {
    records: Vec<TableRecord>,
}

/// Result of every write tool: the staged change (with its diff, so the agent
/// can self-review what was written) and, unless it was a dry run, the commit
/// outcome.
#[derive(Serialize)]
struct WriteOutput {
    change: PendingChange,
    committed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<changes::CommitOutcome>,
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

/// Resolves the effective source for a call (see the module docs).
fn resolve(
    conn: &Connection,
    source_id: Option<&str>,
    table_id: Option<&str>,
) -> Result<String, CoreError> {
    google::resolve_source(conn, source_id, table_id)
}

/// What a write tool reports about itself once its change is staged.
struct WriteCall<'a> {
    /// Audit action name (the tool name).
    action: &'static str,
    source_id: &'a str,
    table_id: Option<&'a str>,
    dry_run: bool,
    /// Tool-specific audit metadata; `changeId` and `dryRun` are added.
    metadata: Value,
}

/// Finishes a write tool after staging: records its audit event, then commits
/// the change unless this is a dry run. The audit event is written before the
/// commit so the call is logged even when applying it fails.
fn finish_write(
    conn: &Connection,
    registry: &ConnectorRegistry,
    call: WriteCall<'_>,
    change: PendingChange,
) -> Result<String, CoreError> {
    let mut metadata = call.metadata;
    metadata["changeId"] = json!(change.id);
    metadata["dryRun"] = json!(call.dry_run);
    audit::record(
        conn,
        AuditActor::Agent,
        call.action,
        Some(call.source_id),
        call.table_id,
        Some(&metadata),
    )?;
    if call.dry_run {
        return pretty(&WriteOutput {
            change,
            committed: false,
            outcome: None,
        });
    }
    // The change stays staged when applying it fails; name it so the agent
    // can retry with commit_change instead of staging a duplicate.
    let outcome = changes::commit(conn, registry, &change.id).map_err(|error| {
        error.map_message(|message| {
            format!(
                "{message} (change {} is still staged; retry with commit_change)",
                change.id
            )
        })
    })?;
    pretty(&WriteOutput {
        change,
        committed: true,
        outcome: Some(outcome),
    })
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
        let source_id = resolve(conn, args.source_id.as_deref(), None)?;
        permissions::assert_can_read(conn, &source_id, None)?;
        let tables = registry.list_tables(conn, &source_id)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "list_tables",
            Some(&source_id),
            None,
            Some(&json!({ "count": tables.len() })),
        )?;
        pretty(&TablesOutput { tables })
    })
}

/// The tabs of one spreadsheet. `tableId` may be any accepted form; only its
/// spreadsheet id is used.
pub fn list_sheets(state: &BrokerState, args: &SourceTableArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_read(conn, &source_id, Some(&args.table_id))?;
        let spreadsheet_id = parse_spreadsheet_id(&args.table_id)?;
        let tabs = registry.list_sheet_tabs(conn, &source_id, &spreadsheet_id)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "list_sheets",
            Some(&source_id),
            Some(&args.table_id),
            Some(&json!({ "count": tabs.len() })),
        )?;
        pretty(&SheetsOutput {
            spreadsheet_id,
            sheets: tabs
                .into_iter()
                .map(|tab| SheetOutput {
                    gid: tab.gid,
                    title: tab.title,
                })
                .collect(),
        })
    })
}

pub fn describe_table(state: &BrokerState, args: &SourceTableArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_read(conn, &source_id, Some(&args.table_id))?;
        let schema = registry.describe_table(conn, &source_id, &args.table_id)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "describe_table",
            Some(&source_id),
            Some(&args.table_id),
            None,
        )?;
        pretty(&SchemaOutput { schema })
    })
}

pub fn read_table(state: &BrokerState, args: &ReadTableArgs) -> Result<String, CoreError> {
    let (limit, offset) = args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_read(conn, &source_id, Some(&args.table_id))?;
        let options = ReadOptions {
            limit: Some(limit),
            offset: Some(offset),
        };
        let records = registry.read_table(conn, &source_id, &args.table_id, options)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "read_table",
            Some(&source_id),
            Some(&args.table_id),
            Some(&json!({ "limit": limit, "offset": offset, "count": records.len() })),
        )?;
        pretty(&RecordsOutput { records })
    })
}

/// One `read_cells` row: the 1-based sheet row number plus its string cells
/// keyed by A1 column letter.
#[derive(Serialize)]
struct CellsRow {
    row: i64,
    cells: GridRow,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CellsOutput {
    /// A1 column letters present in the window.
    columns: Vec<String>,
    rows: Vec<CellsRow>,
    /// Total rows in the tab (or in the requested range) ignoring
    /// limit/offset.
    total_rows: i64,
}

/// Raw coordinate-level read with NO header/record interpretation - the escape
/// hatch for document-style sheets the record model cannot see. Without a
/// `range` it pages over the whole tab from row 1; with one, only that window
/// is fetched and columns start at the window's first column.
pub fn read_cells(state: &BrokerState, args: &ReadCellsArgs) -> Result<String, CoreError> {
    let (limit, offset, range) = args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_read(conn, &source_id, Some(&args.table_id))?;
        let output = match &range {
            Some(window) => {
                let grid = registry.read_grid_range(
                    conn,
                    &source_id,
                    &args.table_id,
                    window,
                    Some(limit),
                    Some(offset),
                )?;
                CellsOutput {
                    columns: grid.columns,
                    rows: grid
                        .rows
                        .into_iter()
                        .map(|row| CellsRow {
                            row: row.row,
                            cells: row.cells,
                        })
                        .collect(),
                    total_rows: grid.total_rows,
                }
            }
            None => {
                let grid = registry.read_grid(
                    conn,
                    &source_id,
                    &args.table_id,
                    Some(limit),
                    Some(offset),
                )?;
                CellsOutput {
                    columns: grid.columns.into_iter().map(|column| column.id).collect(),
                    rows: grid
                        .rows
                        .into_iter()
                        .enumerate()
                        // Grid rows start at sheet row 1; the window shifts by `offset`.
                        .map(|(index, cells)| CellsRow {
                            row: offset + index as i64 + 1,
                            cells,
                        })
                        .collect(),
                    total_rows: grid.total_rows,
                }
            }
        };
        audit::record(
            conn,
            AuditActor::Agent,
            "read_cells",
            Some(&source_id),
            Some(&args.table_id),
            Some(&json!({
                "limit": limit,
                "offset": offset,
                "range": args.range,
                "count": output.rows.len(),
            })),
        )?;
        pretty(&output)
    })
}

pub fn read_formulas(state: &BrokerState, args: &ReadTableArgs) -> Result<String, CoreError> {
    let (limit, offset) = args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_read(conn, &source_id, Some(&args.table_id))?;
        let options = ReadOptions {
            limit: Some(limit),
            offset: Some(offset),
        };
        let records = registry.read_formulas(conn, &source_id, &args.table_id, options)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "read_formulas",
            Some(&source_id),
            Some(&args.table_id),
            Some(&json!({ "limit": limit, "offset": offset, "count": records.len() })),
        )?;
        pretty(&RecordsOutput { records })
    })
}

pub fn find_records(state: &BrokerState, args: &FindRecordsArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_read(conn, &source_id, Some(&args.table_id))?;
        let records = registry.find_records(conn, &source_id, &args.table_id, &args.query)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "find_records",
            Some(&source_id),
            Some(&args.table_id),
            Some(&json!({ "query": args.query, "count": records.len() })),
        )?;
        pretty(&RecordsOutput { records })
    })
}

pub fn get_table_style(state: &BrokerState, args: &SourceTableArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_read(conn, &source_id, Some(&args.table_id))?;
        let style = registry.read_table_style(conn, &source_id, &args.table_id)?;
        audit::record(
            conn,
            AuditActor::Agent,
            "get_table_style",
            Some(&source_id),
            Some(&args.table_id),
            None,
        )?;
        pretty(&StyleOutput { style })
    })
}

pub fn update_records(state: &BrokerState, args: UpdateRecordsArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        // Read permission is checked first: the diff exposes current record
        // values (parity with the TypeScript tool).
        permissions::assert_can_read(conn, &source_id, Some(&args.table_id))?;
        let action = if args.patches.len() > BULK_UPDATE_THRESHOLD {
            WriteAction::BulkUpdate
        } else {
            WriteAction::Update
        };
        permissions::assert_can_write(conn, &source_id, &args.table_id, action)?;
        let patches: Vec<RecordPatch> = args
            .patches
            .into_iter()
            .map(|patch| RecordPatch {
                record_id: patch.record_id,
                fields: patch.fields,
            })
            .collect();
        let patch_count = patches.len();
        let change =
            changes::create_update_change(conn, registry, &source_id, &args.table_id, patches)?;
        finish_write(
            conn,
            registry,
            WriteCall {
                action: "update_records",
                source_id: &source_id,
                table_id: Some(&args.table_id),
                dry_run: args.dry_run,
                metadata: json!({ "patchCount": patch_count }),
            },
            change,
        )
    })
}

pub fn append_records(state: &BrokerState, args: AppendRecordsArgs) -> Result<String, CoreError> {
    let format = args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_write(conn, &source_id, &args.table_id, WriteAction::Append)?;
        // A bundled format plan is also a write.
        if format.is_some() {
            permissions::assert_can_write(conn, &source_id, &args.table_id, WriteAction::Format)?;
        }
        let record_count = args.records.len();
        let has_format = format.is_some();
        let change = changes::create_append_with_format(
            conn,
            &source_id,
            &args.table_id,
            args.records,
            format,
        )?;
        finish_write(
            conn,
            registry,
            WriteCall {
                action: "append_records",
                source_id: &source_id,
                table_id: Some(&args.table_id),
                dry_run: args.dry_run,
                metadata: json!({ "recordCount": record_count, "hasFormat": has_format }),
            },
            change,
        )
    })
}

pub fn update_cells(state: &BrokerState, args: UpdateCellsArgs) -> Result<String, CoreError> {
    let cells = args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_write(conn, &source_id, &args.table_id, WriteAction::Update)?;
        let cell_count = cells.len();
        let change = changes::create_update_cells_change(conn, &source_id, &args.table_id, cells)?;
        finish_write(
            conn,
            registry,
            WriteCall {
                action: "update_cells",
                source_id: &source_id,
                table_id: Some(&args.table_id),
                dry_run: args.dry_run,
                metadata: json!({ "cellCount": cell_count }),
            },
            change,
        )
    })
}

pub fn format_table(state: &BrokerState, args: FormatTableArgs) -> Result<String, CoreError> {
    let plan = args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_write(conn, &source_id, &args.table_id, WriteAction::Format)?;
        let format_count = plan.formats.len();
        let change = changes::create_format_change(conn, &source_id, &args.table_id, plan)?;
        finish_write(
            conn,
            registry,
            WriteCall {
                action: "format_table",
                source_id: &source_id,
                table_id: Some(&args.table_id),
                dry_run: args.dry_run,
                metadata: json!({ "formatCount": format_count }),
            },
            change,
        )
    })
}

pub fn create_spreadsheet(
    state: &BrokerState,
    args: CreateSpreadsheetArgs,
) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), None)?;
        // Source-level: the empty table_id resolves to the source-wide rule.
        permissions::assert_can_write(conn, &source_id, "", WriteAction::CreateSpreadsheet)?;
        let change =
            changes::create_create_spreadsheet_change(conn, &source_id, args.title.clone())?;
        finish_write(
            conn,
            registry,
            WriteCall {
                action: "create_spreadsheet",
                source_id: &source_id,
                table_id: None,
                dry_run: args.dry_run,
                metadata: json!({ "title": args.title }),
            },
            change,
        )
    })
}

pub fn create_sheet(state: &BrokerState, args: CreateSheetArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        permissions::assert_can_write(conn, &source_id, &args.table_id, WriteAction::CreateSheet)?;
        let change = changes::create_create_sheet_change(
            conn,
            &source_id,
            &args.table_id,
            args.title.clone(),
        )?;
        finish_write(
            conn,
            registry,
            WriteCall {
                action: "create_sheet",
                source_id: &source_id,
                table_id: Some(&args.table_id),
                dry_run: args.dry_run,
                metadata: json!({ "title": args.title }),
            },
            change,
        )
    })
}

pub fn delete_sheet(state: &BrokerState, args: DeleteSheetArgs) -> Result<String, CoreError> {
    args.validate()?;
    state.with_conn(|conn, registry| {
        let source_id = resolve(conn, args.source_id.as_deref(), Some(&args.table_id))?;
        // DeleteSheet needs the delete_records permission, so this refuses
        // unless the user has explicitly enabled deletes.
        permissions::assert_can_write(conn, &source_id, &args.table_id, WriteAction::DeleteSheet)?;
        let change = changes::create_delete_sheet_change(conn, &source_id, &args.table_id)?;
        finish_write(
            conn,
            registry,
            WriteCall {
                action: "delete_sheet",
                source_id: &source_id,
                table_id: Some(&args.table_id),
                dry_run: args.dry_run,
                metadata: json!({}),
            },
            change,
        )
    })
}

pub fn commit_change(state: &BrokerState, args: &CommitChangeArgs) -> Result<String, CoreError> {
    let change_ids = args.ids()?;
    state.with_conn(|conn, registry| {
        // Pre-check each change's write permission with the raw change type
        // (parity with the TypeScript tool); changes::commit re-checks with the
        // exact staged action, including the bulk_update escalation.
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
        // Cell writes are updates for permission purposes.
        ChangeType::UpdateCells => WriteAction::Update,
        ChangeType::CreateSpreadsheet => WriteAction::CreateSpreadsheet,
        ChangeType::CreateSheet => WriteAction::CreateSheet,
        ChangeType::DeleteSheet => WriteAction::DeleteSheet,
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
