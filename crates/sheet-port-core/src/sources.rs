//! Reads and maintains the `sources` table shared by every connector and the
//! desktop UI. Fresh databases have no rows; connecting a provider (e.g.
//! Google Sheets) upserts one.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{db_error, CoreError};
use crate::types::{DataSource, SourceKind};

/// `sources.status` value for a linked, working source (see schema CHECK).
pub const SOURCE_STATUS_CONNECTED: &str = "connected";

pub fn list(conn: &Connection) -> Result<Vec<DataSource>, CoreError> {
    let mut stmt = conn
        .prepare("SELECT id, kind, name, status FROM sources ORDER BY id")
        .map_err(|error| db_error("Could not list sources", error))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| db_error("Could not list sources", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list sources", error))?;

    rows.into_iter()
        .map(|(id, kind, name, status)| {
            let kind = parse_kind(&id, &kind)?;
            Ok(DataSource {
                id,
                kind,
                name,
                status: Some(status),
            })
        })
        .collect()
}

pub fn get_kind(conn: &Connection, source_id: &str) -> Result<Option<SourceKind>, CoreError> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT kind FROM sources WHERE id = ?1",
            [source_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| db_error("Could not read source kind", error))?;
    raw.map(|kind| parse_kind(source_id, &kind)).transpose()
}

/// Inserts or refreshes a source row; used when a provider is (re)connected.
pub fn upsert(
    conn: &Connection,
    id: &str,
    kind: SourceKind,
    name: &str,
    status: &str,
) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO sources (id, kind, name, status) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
           kind = excluded.kind, name = excluded.name, status = excluded.status",
        params![id, kind.as_str(), name, status],
    )
    .map_err(|error| db_error("Could not upsert source", error))?;
    Ok(())
}

/// Removes a source row; false when the id did not exist.
pub fn delete(conn: &Connection, id: &str) -> Result<bool, CoreError> {
    let deleted = conn
        .execute("DELETE FROM sources WHERE id = ?1", [id])
        .map_err(|error| db_error("Could not delete source", error))?;
    Ok(deleted > 0)
}

fn parse_kind(source_id: &str, raw: &str) -> Result<SourceKind, CoreError> {
    SourceKind::from_db(raw)
        .ok_or_else(|| CoreError::Storage(format!("Source {source_id} has unknown kind '{raw}'")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::open_temp_db;

    #[test]
    fn fresh_database_has_no_sources() {
        let conn = open_temp_db();
        assert!(list(&conn).expect("list").is_empty());
    }

    #[test]
    fn lists_inserted_sources_ordered_by_id() {
        let conn = open_temp_db();
        upsert(
            &conn,
            "b-mock",
            SourceKind::Mock,
            "Demo Workspace",
            SOURCE_STATUS_CONNECTED,
        )
        .expect("insert mock");
        upsert(
            &conn,
            "a-google",
            SourceKind::GoogleSheets,
            "Google Sheets (user@example.com)",
            SOURCE_STATUS_CONNECTED,
        )
        .expect("insert google");

        let sources = list(&conn).expect("list sources");
        let ids: Vec<&str> = sources.iter().map(|source| source.id.as_str()).collect();
        assert_eq!(ids, ["a-google", "b-mock"]);
        assert_eq!(sources[0].kind, SourceKind::GoogleSheets);
        assert_eq!(sources[1].kind, SourceKind::Mock);
        assert_eq!(
            sources[0].status.as_deref(),
            Some(SOURCE_STATUS_CONNECTED),
            "status column must round-trip"
        );
    }

    #[test]
    fn upsert_updates_existing_row_in_place() {
        let conn = open_temp_db();
        upsert(
            &conn,
            "google-sheets",
            SourceKind::GoogleSheets,
            "Google Sheets (old@example.com)",
            SOURCE_STATUS_CONNECTED,
        )
        .expect("insert");
        upsert(
            &conn,
            "google-sheets",
            SourceKind::GoogleSheets,
            "Google Sheets (new@example.com)",
            SOURCE_STATUS_CONNECTED,
        )
        .expect("update");

        let sources = list(&conn).expect("list");
        assert_eq!(sources.len(), 1, "same id must update, not duplicate");
        assert_eq!(sources[0].name, "Google Sheets (new@example.com)");
    }

    #[test]
    fn delete_reports_whether_a_row_was_removed() {
        let conn = open_temp_db();
        upsert(
            &conn,
            "google-sheets",
            SourceKind::GoogleSheets,
            "Google Sheets (user@example.com)",
            SOURCE_STATUS_CONNECTED,
        )
        .expect("insert");

        assert!(delete(&conn, "google-sheets").expect("first delete"));
        assert!(!delete(&conn, "google-sheets").expect("second delete"));
        assert!(list(&conn).expect("list").is_empty());
    }

    #[test]
    fn get_kind_resolves_known_and_unknown_sources() {
        let conn = open_temp_db();
        upsert(
            &conn,
            "mock-source",
            SourceKind::Mock,
            "Demo Workspace",
            SOURCE_STATUS_CONNECTED,
        )
        .expect("insert");

        assert_eq!(
            get_kind(&conn, "mock-source").expect("kind"),
            Some(SourceKind::Mock)
        );
        assert_eq!(get_kind(&conn, "does-not-exist").expect("kind"), None);
    }
}
