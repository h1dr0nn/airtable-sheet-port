//! Result files for large MCP reads (`saveTo`, docs/mcp-tools.md). A read
//! with `saveTo` writes its full JSON result into the exports directory next
//! to the SQLite database and returns only the file path, so bulk data goes
//! to disk instead of into an agent's context. The caller only ever picks a
//! bare file name; [`export_path`] guarantees the file stays inside that
//! directory.

use std::path::{Path, PathBuf};

use crate::error::CoreError;

/// Directory name of the exports directory, beside the database file.
pub const EXPORTS_DIR_NAME: &str = "exports";
/// Longest accepted `saveTo` file name.
pub const EXPORT_NAME_MAX_LEN: usize = 100;
const EXPORT_EXTENSION: &str = ".json";
/// Windows device names, which name a device instead of a file even with an
/// extension (`NUL.json`).
const RESERVED_STEMS: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The exports directory for a database path: `exports` beside the database
/// file (so `SHEET_PORT_DB` isolates it too).
pub fn exports_dir_for(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(EXPORTS_DIR_NAME)
}

/// Checks a `saveTo` value: a bare file name of 1 to 100 characters from
/// `A-Z a-z 0-9 . _ -`, ending in `.json`, with a non-empty name before the
/// extension, no `..`, and not a Windows device name.
pub fn validate_export_name(name: &str) -> Result<(), CoreError> {
    let bad = |reason: &str| {
        CoreError::InvalidInput(format!(
            "saveTo '{name}' {reason}: pass a file name only, like levels.json (1-{EXPORT_NAME_MAX_LEN} characters of A-Z, a-z, 0-9, '.', '_', '-', ending in .json)"
        ))
    };
    if name.is_empty() || name.len() > EXPORT_NAME_MAX_LEN {
        return Err(bad("has the wrong length"));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(bad("must not contain path separators or other characters"));
    }
    if name.contains("..") {
        return Err(bad("must not contain '..'"));
    }
    let Some(stem) = name.strip_suffix(EXPORT_EXTENSION) else {
        return Err(bad("must end in .json"));
    };
    if stem.is_empty() {
        return Err(bad("needs a name before .json"));
    }
    let device = stem.split('.').next().unwrap_or(stem);
    if RESERVED_STEMS
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(device))
    {
        return Err(bad("is a reserved device name"));
    }
    Ok(())
}

/// The file a `saveTo` name maps to inside `dir`. Validates the name, then
/// double-checks that the joined path is a direct child of `dir`.
pub fn export_path(dir: &Path, name: &str) -> Result<PathBuf, CoreError> {
    validate_export_name(name)?;
    let path = dir.join(name);
    if path.parent() != Some(dir) || path.file_name().and_then(|file| file.to_str()) != Some(name) {
        return Err(CoreError::InvalidInput(format!(
            "saveTo '{name}' must be a file name only"
        )));
    }
    Ok(path)
}

/// Writes `bytes` to the `saveTo` file in `dir` (created if missing,
/// replacing a file of the same name) and returns its absolute path.
pub fn write_export(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, CoreError> {
    let dir = std::path::absolute(dir).map_err(|error| {
        CoreError::Storage(format!("Could not resolve the exports directory: {error}"))
    })?;
    let path = export_path(&dir, name)?;
    std::fs::create_dir_all(&dir).map_err(|error| {
        CoreError::Storage(format!("Could not create the exports directory: {error}"))
    })?;
    std::fs::write(&path, bytes).map_err(|error| {
        CoreError::Storage(format!("Could not write {}: {error}", path.display()))
    })?;
    Ok(path)
}

#[cfg(test)]
#[path = "exports_tests.rs"]
mod tests;
