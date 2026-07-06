//! Reads the `sources` table shared by every connector and the desktop UI.

use rusqlite::{Connection, OptionalExtension};

use crate::error::{db_error, CoreError};
use crate::types::{DataSource, SourceKind};

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

fn parse_kind(source_id: &str, raw: &str) -> Result<SourceKind, CoreError> {
    SourceKind::from_db(raw)
        .ok_or_else(|| CoreError::Storage(format!("Source {source_id} has unknown kind '{raw}'")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::open_temp_db;

    #[test]
    fn lists_seeded_sources_ordered_by_id() {
        let conn = open_temp_db();
        let sources = list(&conn).expect("list sources");
        let ids: Vec<&str> = sources.iter().map(|source| source.id.as_str()).collect();
        assert_eq!(
            ids,
            ["google-placeholder", "mock-source", "provider-placeholder"]
        );
        assert_eq!(sources[1].kind, SourceKind::Mock);
        assert_eq!(sources[1].status.as_deref(), Some("connected"));
    }

    #[test]
    fn get_kind_resolves_known_and_unknown_sources() {
        let conn = open_temp_db();
        assert_eq!(
            get_kind(&conn, "mock-source").expect("kind"),
            Some(SourceKind::Mock)
        );
        assert_eq!(get_kind(&conn, "does-not-exist").expect("kind"), None);
    }
}
