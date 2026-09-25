//! Shared broker state for the sidecar: the single SQLite connection, the
//! connector registry, and who runs this sidecar (heartbeat identity). rusqlite's `Connection` is Send but not Sync, so tool
//! handlers and the heartbeat task funnel through one mutex. Connector calls
//! may block on HTTP (Google Sheets), which is why the server layer runs
//! every tool body on `spawn_blocking` instead of the async runtime threads.

use std::sync::Mutex;

use sheet_port_core::connectors::ConnectorRegistry;
use sheet_port_core::heartbeat::{self, HeartbeatIdentity};
use sheet_port_core::rusqlite::Connection;
use sheet_port_core::CoreError;

/// Written to our heartbeat row so the desktop can flag a sidecar left
/// running from an older install (the MCP client must restart to pick up
/// the new binary).
pub const SIDECAR_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct BrokerState {
    conn: Mutex<Connection>,
    registry: ConnectorRegistry,
    /// Exe paths from startup plus the client info from `initialize`;
    /// rewritten on every heartbeat so a deleted row comes back complete.
    identity: Mutex<HeartbeatIdentity>,
}

impl BrokerState {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
            registry: ConnectorRegistry::with_default_connectors(),
            identity: Mutex::new(HeartbeatIdentity::default()),
        }
    }

    /// Merges the known fields of `update` into the heartbeat identity.
    pub fn update_identity(&self, update: HeartbeatIdentity) {
        let Ok(mut identity) = self.identity.lock() else {
            return;
        };
        let merge = |slot: &mut Option<String>, value: Option<String>| {
            if value.is_some() {
                *slot = value;
            }
        };
        merge(&mut identity.client_name, update.client_name);
        merge(&mut identity.client_version, update.client_version);
        merge(&mut identity.exe_path, update.exe_path);
        merge(&mut identity.parent_exe_path, update.parent_exe_path);
    }

    /// Upserts this sidecar's heartbeat row with the current identity.
    pub fn write_heartbeat(&self, pid: i64) -> Result<(), CoreError> {
        let identity = self
            .identity
            .lock()
            .map(|identity| identity.clone())
            .unwrap_or_default();
        self.with_conn(|conn, _| {
            heartbeat::upsert_own_with_identity(conn, pid, SIDECAR_VERSION, &identity)
        })
    }

    /// Runs `task` while holding the shared connection. A poisoned mutex is
    /// reported as a storage error instead of panicking so a single failed
    /// call can never take the whole sidecar down.
    pub fn with_conn<T>(
        &self,
        task: impl FnOnce(&Connection, &ConnectorRegistry) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        let conn = self.conn.lock().map_err(|_| {
            CoreError::Storage("Shared database connection lock is poisoned".to_string())
        })?;
        task(&conn, &self.registry)
    }
}
