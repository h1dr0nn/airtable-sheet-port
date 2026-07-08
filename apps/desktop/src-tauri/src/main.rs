// Release builds must NOT allocate a console: this is a GUI app, and a console
// subsystem binary would pop a terminal window on launch (and surface stray
// stdout/stderr in it). Debug keeps the console so `cargo run` logs are visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sheet_port_lib::run()
}
