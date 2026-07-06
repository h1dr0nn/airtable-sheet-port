//! Integration tests for the optional HTTP transport. They spawn the built
//! sidecar binary with `SHEET_PORT_MCP_TRANSPORT=http` against an isolated temp
//! database and assert the two contract guarantees that unit tests cannot
//! cover: the endpoint is reachable on 127.0.0.1 only, and a taken port makes
//! the process exit non-zero.
//!
//! These tests require the debug binary. Cargo builds it automatically for
//! integration tests via the `CARGO_BIN_EXE_<name>` env var.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const BIND_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Absolute path to the freshly built sidecar binary.
fn sidecar_binary() -> &'static str {
    env!("CARGO_BIN_EXE_sheet-port-mcp")
}

/// A unique temp DB path per test so runs never share state.
fn temp_db_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "sheet-port-mcp-http-{tag}-{}.db",
        uuid::Uuid::new_v4()
    ))
}

/// An OS-assigned free port. Bind to :0, read the port, then drop the listener
/// so the sidecar can claim it. There is a small TOCTOU window, but the loopback
/// space is large enough that this is reliable for a test.
fn free_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral");
    listener.local_addr().expect("local addr").port()
}

fn spawn_http_sidecar(db_path: &std::path::Path, port: u16) -> Child {
    Command::new(sidecar_binary())
        .env("SHEET_PORT_DB", db_path)
        .env("SHEET_PORT_MCP_TRANSPORT", "http")
        .env("SHEET_PORT_MCP_PORT", port.to_string())
        .spawn()
        .expect("spawn sidecar")
}

/// Polls until a TCP connection to 127.0.0.1:port succeeds or the timeout hits.
fn wait_for_listener(port: u16) -> bool {
    let deadline = Instant::now() + BIND_TIMEOUT;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    false
}

fn cleanup(db_path: &std::path::Path) {
    let _ = std::fs::remove_file(db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

#[test]
fn http_transport_serves_on_loopback() {
    let db_path = temp_db_path("serves");
    let port = free_port();
    let mut child = spawn_http_sidecar(&db_path, port);

    assert!(
        wait_for_listener(port),
        "sidecar did not start listening on 127.0.0.1:{port} within timeout"
    );

    // A bare GET (no MCP session) still gets an HTTP response, proving the
    // server is up rather than connection-refused. We only care that we get
    // an HTTP/1.1 status line back.
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect loopback");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    stream
        .write_all(
            format!("GET /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .expect("write request");
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    assert!(
        response.starts_with("HTTP/1.1"),
        "expected an HTTP response, got: {response:?}"
    );

    let _ = child.kill();
    let _ = child.wait();
    cleanup(&db_path);
}

#[test]
fn http_transport_exits_nonzero_when_port_taken() {
    let db_path = temp_db_path("conflict");
    // Hold the port for the whole test so the sidecar cannot bind it.
    let holder = TcpListener::bind(("127.0.0.1", 0)).expect("hold port");
    let port = holder.local_addr().expect("addr").port();

    let mut child = spawn_http_sidecar(&db_path, port);
    let status = child.wait().expect("sidecar should exit on bind failure");
    assert!(
        !status.success(),
        "sidecar must exit non-zero when the port is already in use"
    );

    drop(holder);
    cleanup(&db_path);
}
