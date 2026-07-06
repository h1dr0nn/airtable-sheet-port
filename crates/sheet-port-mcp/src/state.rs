//! Shared broker state for the sidecar: the single SQLite connection plus the
//! connector registry. rusqlite's `Connection` is Send but not Sync, so tool
//! handlers and the heartbeat task funnel through one mutex; every operation
//! is a short synchronous SQLite call, so contention stays negligible.

use std::sync::Mutex;

use sheet_port_core::connectors::ConnectorRegistry;
use sheet_port_core::rusqlite::Connection;
use sheet_port_core::CoreError;

pub struct BrokerState {
    conn: Mutex<Connection>,
    registry: ConnectorRegistry,
}

impl BrokerState {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
            registry: ConnectorRegistry::with_default_connectors(),
        }
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
