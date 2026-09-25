//! Embeds a Windows version resource into sheet-port-mcp.exe so Explorer and
//! `(Get-Item sheet-port-mcp.exe).VersionInfo` show which build is installed.
//! Other targets (including cross-compiles from Windows) skip it: the check is
//! on the target OS at build-script runtime, not on the host.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // FileVersion/ProductVersion and the numeric version fields default to
    // CARGO_PKG_VERSION in tauri-winres.
    let mut res = tauri_winres::WindowsResource::new();
    res.set("FileDescription", "Airtable - Sheet Port MCP sidecar")
        .set("ProductName", "Airtable - Sheet Port")
        .set("OriginalFilename", "sheet-port-mcp.exe")
        .set("InternalName", "sheet-port-mcp");
    if let Err(error) = res.compile_for(&["sheet-port-mcp"]) {
        panic!("could not embed the Windows version resource: {error}");
    }
}
