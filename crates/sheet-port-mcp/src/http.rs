//! Optional loopback HTTP transport for the MCP sidecar.
//!
//! Serves the exact same [`SheetPortServer`] tools as the stdio transport, but
//! over rmcp's streamable-http server (a `tower` Service) driven by hyper on a
//! TcpListener. SECURITY: the listener is bound STRICTLY to 127.0.0.1 and the
//! transport's `allowed_hosts` keeps its loopback-only default, so the endpoint
//! is never reachable from another machine (see docs/security.md). Binding
//! 0.0.0.0 is intentionally impossible here.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as ConnBuilder;
use hyper_util::service::TowerToHyperService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;

use crate::logging::log;
use crate::server::SheetPortServer;
use crate::state::BrokerState;

/// The single MCP endpoint path clients POST to. Kept simple and documented so
/// the desktop can advertise `http://127.0.0.1:{port}{MCP_HTTP_PATH}`.
pub const MCP_HTTP_PATH: &str = "/mcp";

/// Serves the MCP tools over 127.0.0.1:{port} until `shutdown` resolves.
///
/// Returns an error (so `main` exits non-zero) when the port cannot be bound -
/// typically because it is already in use. Each accepted connection is served
/// on its own task; the streamable-http service spawns an MCP session per
/// client and shares the same broker state as the stdio path.
pub async fn serve<F>(
    state: Arc<BrokerState>,
    port: u16,
    shutdown: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: std::future::Future<Output = ()>,
{
    // Hard rule: loopback only. The address is constructed from LOCALHOST, so
    // there is no code path that binds a routable interface.
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = TcpListener::bind(addr).await.map_err(|error| {
        // Surface the bound address so "port in use" is obvious in the log.
        format!("could not bind MCP HTTP transport to {addr}: {error}")
    })?;

    // A fresh SheetPortServer per session; all share the same Arc<BrokerState>.
    let service_state = Arc::clone(&state);
    let service = StreamableHttpService::new(
        move || Ok(SheetPortServer::new(Arc::clone(&service_state))),
        Arc::new(LocalSessionManager::default()),
        // Default config already restricts allowed_hosts to loopback; keep it.
        StreamableHttpServerConfig::default(),
    );

    log(&format!(
        "http transport listening on http://{addr}{MCP_HTTP_PATH}"
    ));

    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                log("http transport shutting down");
                return Ok(());
            }
            accepted = listener.accept() => {
                let (stream, _peer) = match accepted {
                    Ok(pair) => pair,
                    Err(error) => {
                        // A transient accept error must not kill the server.
                        log(&format!("http accept failed: {error}"));
                        continue;
                    }
                };
                // TowerToHyperService already implements hyper's Service for
                // Request<Incoming>, since the rmcp tower service is generic
                // over any http_body::Body (Incoming qualifies).
                let hyper_service = TowerToHyperService::new(service.clone());
                tokio::spawn(async move {
                    let io = TokioIo::new(stream);
                    let builder = ConnBuilder::new(TokioExecutor::new());
                    if let Err(error) = builder.serve_connection(io, hyper_service).await {
                        log(&format!("http connection error: {error}"));
                    }
                });
            }
        }
    }
}
