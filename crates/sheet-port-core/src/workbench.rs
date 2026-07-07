//! Workbench: a user-curated tree of spreadsheets grouped into folders,
//! distinct from the raw connector `list_tables` path. Folders and items live
//! in the shared SQLite database (workbench_folders / workbench_items); every
//! mutation records an audit event. Sheet reads and writes go through the
//! connector registry's grid methods (see connectors/mod.rs).

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::audit;
use crate::connectors::{parse_spreadsheet_id, spreadsheet_title, ConnectorRegistry};
use crate::error::{db_error, CoreError};
use crate::types::{AuditActor, SourceKind};

/// Id prefixes so a raw id reveals its kind at a glance.
const FOLDER_ID_PREFIX: &str = "wbf_";
const ITEM_ID_PREFIX: &str = "wbi_";

/// Audit actions for every Workbench mutation (actor = user).
const ACTION_FOLDER_CREATED: &str = "workbench_folder_created";
const ACTION_FOLDER_RENAMED: &str = "workbench_folder_renamed";
const ACTION_FOLDER_DELETED: &str = "workbench_folder_deleted";
const ACTION_ITEM_ADDED: &str = "workbench_item_added";
const ACTION_ITEM_REMOVED: &str = "workbench_item_removed";
const ACTION_ITEM_MOVED: &str = "workbench_item_moved";

/// A user-created folder that groups spreadsheets in the Workbench tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchFolder {
    pub id: String,
    pub name: String,
    /// Ascending sort order within the tree.
    pub position: i64,
}

/// One spreadsheet the user has added to the Workbench, optionally foldered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchItem {
    pub id: String,
    /// `None` -> shown under "Ungrouped".
    pub folder_id: Option<String>,
    /// Owning data source (a connected Google account).
    pub source_id: String,
    pub spreadsheet_id: String,
    /// Resolved display name (the spreadsheet title).
    pub name: String,
    /// Ascending sort order within its folder.
    pub position: i64,
}

/// The full curated tree: every folder plus every added spreadsheet.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchTree {
    pub folders: Vec<WorkbenchFolder>,
    pub items: Vec<WorkbenchItem>,
}

/// The whole tree: folders ordered by position then name, items by position.
pub fn tree(conn: &Connection) -> Result<WorkbenchTree, CoreError> {
    Ok(WorkbenchTree {
        folders: list_folders(conn)?,
        items: list_items(conn)?,
    })
}

/// Creates a folder at the end of the tree (position = max + 1). The name is
/// trimmed and must not be empty.
pub fn create_folder(conn: &Connection, name: &str) -> Result<WorkbenchFolder, CoreError> {
    let name = validate_folder_name(name)?;
    let folder = WorkbenchFolder {
        id: format!("{FOLDER_ID_PREFIX}{}", uuid::Uuid::new_v4()),
        name,
        position: next_folder_position(conn)?,
    };
    conn.execute(
        "INSERT INTO workbench_folders (id, name, position) VALUES (?1, ?2, ?3)",
        params![folder.id, folder.name, folder.position],
    )
    .map_err(|error| db_error("Could not create workbench folder", error))?;
    record_event(
        conn,
        ACTION_FOLDER_CREATED,
        None,
        None,
        &json!({ "id": folder.id, "name": folder.name }),
    )?;
    Ok(folder)
}

/// Renames a folder in place. Errors when the folder id does not exist.
pub fn rename_folder(conn: &Connection, id: &str, name: &str) -> Result<(), CoreError> {
    let name = validate_folder_name(name)?;
    let changed = conn
        .execute(
            "UPDATE workbench_folders SET name = ?1 WHERE id = ?2",
            params![name, id],
        )
        .map_err(|error| db_error("Could not rename workbench folder", error))?;
    if changed == 0 {
        return Err(unknown_folder(id));
    }
    record_event(
        conn,
        ACTION_FOLDER_RENAMED,
        None,
        None,
        &json!({ "id": id, "name": name }),
    )?;
    Ok(())
}

/// Deletes a folder; its spreadsheets fall back to Ungrouped via the
/// `ON DELETE SET NULL` foreign key (foreign_keys must be ON). Errors when the
/// folder id does not exist.
pub fn delete_folder(conn: &Connection, id: &str) -> Result<(), CoreError> {
    let changed = conn
        .execute("DELETE FROM workbench_folders WHERE id = ?1", [id])
        .map_err(|error| db_error("Could not delete workbench folder", error))?;
    if changed == 0 {
        return Err(unknown_folder(id));
    }
    record_event(
        conn,
        ACTION_FOLDER_DELETED,
        None,
        None,
        &json!({ "id": id }),
    )?;
    Ok(())
}

/// Resolves a pasted URL/id into a spreadsheet and adds it to the tree. The
/// source is the first connected Google account (a clear error when none is
/// connected); the display name is the spreadsheet's own title. If the same
/// spreadsheet already exists in the target folder, the existing item is
/// returned unchanged (no duplicate, no extra network call).
pub fn add_spreadsheet(
    conn: &Connection,
    registry: &ConnectorRegistry,
    folder_id: Option<&str>,
    url_or_id: &str,
) -> Result<WorkbenchItem, CoreError> {
    let source_id = first_connected_google_source(conn, registry)?;
    let spreadsheet_id = parse_spreadsheet_id(url_or_id)?;

    if let Some(existing) = find_item_in_folder(conn, folder_id, &spreadsheet_id)? {
        return Ok(existing);
    }
    if let Some(fid) = folder_id {
        if !folder_exists(conn, fid)? {
            return Err(unknown_folder(fid));
        }
    }

    let name = spreadsheet_title(conn, &source_id, &spreadsheet_id)?;
    let item = WorkbenchItem {
        id: format!("{ITEM_ID_PREFIX}{}", uuid::Uuid::new_v4()),
        folder_id: folder_id.map(str::to_string),
        source_id,
        spreadsheet_id,
        name,
        position: next_item_position(conn, folder_id)?,
    };
    conn.execute(
        "INSERT INTO workbench_items
           (id, folder_id, source_id, spreadsheet_id, name, position)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            item.id,
            item.folder_id,
            item.source_id,
            item.spreadsheet_id,
            item.name,
            item.position
        ],
    )
    .map_err(|error| db_error("Could not add workbench spreadsheet", error))?;
    record_event(
        conn,
        ACTION_ITEM_ADDED,
        Some(&item.source_id),
        Some(&item.spreadsheet_id),
        &json!({ "id": item.id, "name": item.name }),
    )?;
    Ok(item)
}

/// Removes one spreadsheet from the Workbench (does not touch the source).
/// Errors when the item id does not exist.
pub fn remove_item(conn: &Connection, id: &str) -> Result<(), CoreError> {
    let item = get_item(conn, id)?;
    conn.execute("DELETE FROM workbench_items WHERE id = ?1", [id])
        .map_err(|error| db_error("Could not remove workbench item", error))?;
    record_event(
        conn,
        ACTION_ITEM_REMOVED,
        Some(&item.source_id),
        Some(&item.spreadsheet_id),
        &json!({ "id": id }),
    )?;
    Ok(())
}

/// Moves a spreadsheet to another folder, or to Ungrouped when `folder_id` is
/// `None`. The item lands at the end of its destination. Errors when the item
/// id or a non-null target folder does not exist.
pub fn move_item(conn: &Connection, id: &str, folder_id: Option<&str>) -> Result<(), CoreError> {
    let item = get_item(conn, id)?;
    if let Some(fid) = folder_id {
        if !folder_exists(conn, fid)? {
            return Err(unknown_folder(fid));
        }
    }
    let position = next_item_position(conn, folder_id)?;
    conn.execute(
        "UPDATE workbench_items SET folder_id = ?1, position = ?2 WHERE id = ?3",
        params![folder_id, position, id],
    )
    .map_err(|error| db_error("Could not move workbench item", error))?;
    record_event(
        conn,
        ACTION_ITEM_MOVED,
        Some(&item.source_id),
        Some(&item.spreadsheet_id),
        &json!({ "id": id, "folderId": folder_id }),
    )?;
    Ok(())
}

/// One item by id, for resolving its source + spreadsheet before a grid call.
/// Errors when the item id does not exist.
pub fn get_item(conn: &Connection, id: &str) -> Result<WorkbenchItem, CoreError> {
    get_item_opt(conn, id)?
        .ok_or_else(|| CoreError::NotFound(format!("Unknown workbench item {id}")))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn validate_folder_name(name: &str) -> Result<String, CoreError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidInput(
            "Folder name must not be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn unknown_folder(id: &str) -> CoreError {
    CoreError::NotFound(format!("Unknown workbench folder {id}"))
}

fn record_event(
    conn: &Connection,
    action: &str,
    source_id: Option<&str>,
    table_id: Option<&str>,
    metadata: &Value,
) -> Result<(), CoreError> {
    audit::record(
        conn,
        AuditActor::User,
        action,
        source_id,
        table_id,
        Some(metadata),
    )?;
    Ok(())
}

fn list_folders(conn: &Connection) -> Result<Vec<WorkbenchFolder>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, position FROM workbench_folders
             ORDER BY position, name",
        )
        .map_err(|error| db_error("Could not list workbench folders", error))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(WorkbenchFolder {
                id: row.get(0)?,
                name: row.get(1)?,
                position: row.get(2)?,
            })
        })
        .map_err(|error| db_error("Could not list workbench folders", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list workbench folders", error))
}

fn list_items(conn: &Connection) -> Result<Vec<WorkbenchItem>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, folder_id, source_id, spreadsheet_id, name, position
             FROM workbench_items ORDER BY position, id",
        )
        .map_err(|error| db_error("Could not list workbench items", error))?;
    let rows = stmt
        .query_map([], item_from_row)
        .map_err(|error| db_error("Could not list workbench items", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list workbench items", error))
}

fn get_item_opt(conn: &Connection, id: &str) -> Result<Option<WorkbenchItem>, CoreError> {
    conn.query_row(
        "SELECT id, folder_id, source_id, spreadsheet_id, name, position
         FROM workbench_items WHERE id = ?1",
        [id],
        item_from_row,
    )
    .optional()
    .map_err(|error| db_error("Could not read workbench item", error))
}

fn item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkbenchItem> {
    Ok(WorkbenchItem {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        source_id: row.get(2)?,
        spreadsheet_id: row.get(3)?,
        name: row.get(4)?,
        position: row.get(5)?,
    })
}

fn folder_exists(conn: &Connection, id: &str) -> Result<bool, CoreError> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM workbench_folders WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| db_error("Could not read workbench folder", error))?;
    Ok(found.is_some())
}

/// The first connected Google account's source id (ordered by id), or a clear
/// error when no Google account is connected.
fn first_connected_google_source(
    conn: &Connection,
    registry: &ConnectorRegistry,
) -> Result<String, CoreError> {
    registry
        .list_sources(conn)?
        .into_iter()
        .find(|source| source.kind == SourceKind::GoogleSheets)
        .map(|source| source.id)
        .ok_or_else(|| {
            CoreError::PermissionDenied(
                "Connect a Google account in the desktop app before adding spreadsheets"
                    .to_string(),
            )
        })
}

/// The existing item for `spreadsheet_id` within `folder_id` (NULL-aware), if
/// any. Used to dedupe adds.
fn find_item_in_folder(
    conn: &Connection,
    folder_id: Option<&str>,
    spreadsheet_id: &str,
) -> Result<Option<WorkbenchItem>, CoreError> {
    let result = match folder_id {
        Some(fid) => conn.query_row(
            "SELECT id, folder_id, source_id, spreadsheet_id, name, position
             FROM workbench_items WHERE folder_id = ?1 AND spreadsheet_id = ?2",
            params![fid, spreadsheet_id],
            item_from_row,
        ),
        None => conn.query_row(
            "SELECT id, folder_id, source_id, spreadsheet_id, name, position
             FROM workbench_items WHERE folder_id IS NULL AND spreadsheet_id = ?1",
            [spreadsheet_id],
            item_from_row,
        ),
    };
    result
        .optional()
        .map_err(|error| db_error("Could not look up workbench item", error))
}

fn next_folder_position(conn: &Connection) -> Result<i64, CoreError> {
    conn.query_row(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM workbench_folders",
        [],
        |row| row.get(0),
    )
    .map_err(|error| db_error("Could not read workbench folder positions", error))
}

fn next_item_position(conn: &Connection, folder_id: Option<&str>) -> Result<i64, CoreError> {
    let position = match folder_id {
        Some(fid) => conn.query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM workbench_items WHERE folder_id = ?1",
            [fid],
            |row| row.get(0),
        ),
        None => conn.query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM workbench_items WHERE folder_id IS NULL",
            [],
            |row| row.get(0),
        ),
    };
    position.map_err(|error| db_error("Could not read workbench item positions", error))
}

#[cfg(test)]
#[path = "workbench_tests.rs"]
mod tests;
