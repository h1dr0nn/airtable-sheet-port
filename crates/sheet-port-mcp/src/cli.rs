//! Headless bridge management: `sheet-port-mcp bridge add|list|remove`. Lets
//! a bridge be added without opening the desktop app (and drives the live
//! smoke). The secret is read from `SHEET_PORT_BRIDGE_SECRET` or, when that is
//! unset, from the first line of stdin, so it never appears in the process
//! list. Output is JSON on stdout; errors go to stderr.

use std::io::BufRead;

use serde_json::json;
use sheet_port_core::types::AuditActor;
use sheet_port_core::{audit, db, google, CoreError};

/// Env var the `bridge add` secret is read from before falling back to stdin.
const ENV_BRIDGE_SECRET: &str = "SHEET_PORT_BRIDGE_SECRET";

const USAGE: &str =
    "usage: sheet-port-mcp bridge add <url> | bridge list | bridge remove <sourceId>";

/// Returns Some(result) when `args` (without the program name) is a CLI
/// command, None when the process should serve MCP as usual.
pub fn try_run(args: &[String]) -> Option<Result<String, CoreError>> {
    if args.first().map(String::as_str) != Some("bridge") {
        return None;
    }
    Some(run_bridge(&args[1..]))
}

fn run_bridge(args: &[String]) -> Result<String, CoreError> {
    let (conn, _) = db::open_default()?;
    match args {
        [command, url] if command == "add" => {
            let secret = read_secret()?;
            let account = google::add_bridge(&conn, url, &secret)?;
            audit::record(
                &conn,
                AuditActor::User,
                "google_bridge_added",
                Some(&account.source_id),
                None,
                Some(&json!({ "via": "cli" })),
            )?;
            to_json(&account)
        }
        [command] if command == "list" => to_json(&google::list_accounts(&conn)?),
        [command, source_id] if command == "remove" => {
            google::remove_bridge(&conn, source_id)?;
            audit::record(
                &conn,
                AuditActor::User,
                "google_bridge_removed",
                Some(source_id),
                None,
                Some(&json!({ "via": "cli" })),
            )?;
            to_json(&json!({ "removed": source_id }))
        }
        _ => Err(CoreError::InvalidInput(USAGE.to_string())),
    }
}

/// The bridge secret from the env var, else the first stdin line.
fn read_secret() -> Result<String, CoreError> {
    if let Ok(secret) = std::env::var(ENV_BRIDGE_SECRET) {
        return Ok(secret);
    }
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|error| CoreError::InvalidInput(format!("Could not read the secret: {error}")))?;
    Ok(line.trim().to_string())
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String, CoreError> {
    serde_json::to_string_pretty(value)
        .map_err(|error| CoreError::Storage(format!("Could not encode output: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_bridge_args_fall_through_to_the_server() {
        assert!(try_run(&[]).is_none());
        assert!(try_run(&["--stdio".to_string()]).is_none());
    }
}
