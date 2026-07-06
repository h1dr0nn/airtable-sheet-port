//! Airtable - Sheet Port MCP sidecar entry point. Serves either the stdio
//! transport (default) or an optional loopback HTTP transport, keeps the
//! mcp_heartbeat row fresh so the desktop app can show sidecar status, and
//! cleans that row up on shutdown. For stdio, stdout belongs to the MCP
//! transport; all diagnostics go to stderr on both transports.
//!
//! The transport and port are read once from the shared `meta` table (or the
//! SHEET_PORT_MCP_TRANSPORT / SHEET_PORT_MCP_PORT env overrides used by tests).
//! Changing the setting requires a sidecar restart to take effect.

mod args;
mod http;
mod logging;
mod server;
mod state;
mod tools;

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use rmcp::service::QuitReason;
use rmcp::transport::stdio;
use rmcp::ServiceExt;
use sheet_port_core::constants::{HEARTBEAT_INTERVAL_MS, HEARTBEAT_STALE_MS};
use sheet_port_core::db::{McpConfig, McpTransport};
use sheet_port_core::{db, heartbeat};

use crate::logging::log;
use crate::server::SheetPortServer;
use crate::state::BrokerState;

/// Env override for the transport, used by tests and power users without
/// touching the desktop settings. Values match the meta strings ("stdio" |
/// "http"); anything else falls back to stdio.
const ENV_TRANSPORT: &str = "SHEET_PORT_MCP_TRANSPORT";
/// Env override for the HTTP port (decimal). Out-of-range or unparseable
/// values clamp to the valid window exactly like the meta value.
const ENV_PORT: &str = "SHEET_PORT_MCP_PORT";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log(&format!("fatal: {error}"));
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (conn, db_path) = db::open_default()?;
    let state = Arc::new(BrokerState::new(conn));
    // i64 matches the mcp_heartbeat.pid column affinity.
    let pid = i64::from(std::process::id());

    let config = resolve_config(&state)?;

    // Heartbeat: the desktop app treats the sidecar as running while this
    // row stays fresh. Clean up rows left behind by crashed processes first.
    // Done before serving so status is accurate on both transports.
    state.with_conn(|conn, _| {
        heartbeat::delete_stale(conn, HEARTBEAT_STALE_MS)?;
        heartbeat::upsert_own(conn, pid)
    })?;
    let heartbeat_task = spawn_heartbeat(Arc::clone(&state), pid);

    log(&format!(
        "ready (pid {pid}, transport {}, db {})",
        config.transport.as_str(),
        db_path.display()
    ));

    // The stdio path is kept byte-identical to the previous implementation so
    // scripts/e2e-smoke.mjs still passes unchanged; HTTP is strictly additive.
    let result = match config.transport {
        McpTransport::Stdio => serve_stdio(Arc::clone(&state)).await,
        McpTransport::Http => http::serve(Arc::clone(&state), config.port, shutdown_signal()).await,
    };

    heartbeat_task.abort();
    // Best effort: the DB may already be unavailable while the process exits.
    if let Err(error) = state.with_conn(|conn, _| heartbeat::delete_own(conn, pid)) {
        log(&format!("heartbeat cleanup failed: {error}"));
    }
    result
}

/// Resolves the transport + port from env overrides first (tests, power
/// users), then the shared `meta` table. Falls back to the stdio default so a
/// missing or unreadable setting never blocks startup.
fn resolve_config(state: &BrokerState) -> Result<McpConfig, Box<dyn std::error::Error>> {
    let env_transport = std::env::var(ENV_TRANSPORT).ok();
    let env_port = std::env::var(ENV_PORT).ok();

    // meta is only consulted for values the env does not override.
    let meta = state.with_conn(|conn, _| db::get_mcp_config(conn))?;

    let transport = match env_transport.as_deref() {
        Some(value) => McpTransport::from_meta_value(Some(value)),
        None => meta.transport,
    };
    let port = match env_port.as_deref() {
        Some(value) => db::clamp_mcp_port(Some(value)),
        None => meta.port,
    };
    Ok(McpConfig { transport, port })
}

/// The original stdio transport: serve, then run until the transport closes or
/// a shutdown signal arrives. Unchanged in behavior from the stdio-only build.
async fn serve_stdio(state: Arc<BrokerState>) -> Result<(), Box<dyn std::error::Error>> {
    let service = SheetPortServer::new(state).serve(stdio()).await?;
    tokio::select! {
        quit = service.waiting() => match quit {
            Ok(QuitReason::Closed) => log("transport closed"),
            Ok(QuitReason::Cancelled) => log("service cancelled"),
            Ok(QuitReason::JoinError(error)) => log(&format!("service join error: {error}")),
            // QuitReason may grow variants; report anything unexpected.
            Ok(other) => log(&format!("service stopped: {other:?}")),
            Err(error) => log(&format!("service task failed: {error}")),
        },
        _ = shutdown_signal() => log("shutdown signal received"),
    }
    Ok(())
}

/// Refreshes our mcp_heartbeat row every HEARTBEAT_INTERVAL_MS. The first
/// refresh already happened synchronously at startup, so the ticker starts
/// one full interval out.
fn spawn_heartbeat(state: Arc<BrokerState>, pid: i64) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let period = Duration::from_millis(HEARTBEAT_INTERVAL_MS.unsigned_abs());
        let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        loop {
            ticker.tick().await;
            if let Err(error) = state.with_conn(|conn, _| heartbeat::upsert_own(conn, pid)) {
                log(&format!("heartbeat update failed: {error}"));
            }
        }
    })
}

/// Resolves on Ctrl+C everywhere and additionally on SIGTERM on Unix,
/// mirroring the SIGINT/SIGTERM handling of the TypeScript sidecar.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(sigterm) => sigterm,
            Err(error) => {
                log(&format!("could not install SIGTERM handler: {error}"));
                wait_for_ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = wait_for_ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        wait_for_ctrl_c().await;
    }
}

async fn wait_for_ctrl_c() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        log(&format!("could not listen for ctrl-c: {error}"));
        // Without a signal handler the transport-closed path still stops the
        // process; park this branch forever instead of busy-looping.
        std::future::pending::<()>().await;
    }
}
