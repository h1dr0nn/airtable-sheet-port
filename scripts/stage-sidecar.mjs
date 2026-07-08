// Builds the sheet-port-mcp sidecar and stages it as a Tauri `externalBin` so
// the installer bundles it next to the desktop executable. Tauri requires the
// file at `apps/desktop/src-tauri/binaries/sheet-port-mcp-<target-triple><ext>`
// to exist before `tauri dev` / `tauri build`; this script produces it.
//
// Run automatically from tauri.conf.json before{Dev,Build}Command. Honors the
// env Tauri injects into those hooks:
//   TAURI_ENV_TARGET_TRIPLE - the triple Tauri is building for (host in dev).
//   TAURI_ENV_DEBUG=true     - dev/debug profile; otherwise release.
// Both have safe fallbacks so the script also works when invoked by hand.

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SIDECAR_CRATE = "sheet-port-mcp";
const BIN_NAME = "sheet-port-mcp";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** Host target triple from `rustc -vV` (the `host:` line). */
function hostTriple() {
  const out = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const match = out.match(/^host:\s*(.+)$/m);
  if (!match) {
    throw new Error("Could not determine host target triple from `rustc -vV`");
  }
  return match[1].trim();
}

const triple = process.env.TAURI_ENV_TARGET_TRIPLE?.trim() || hostTriple();
const isDebug = process.env.TAURI_ENV_DEBUG === "true";
const profile = isDebug ? "debug" : "release";
const ext = triple.includes("windows") ? ".exe" : "";

const cargoArgs = ["build", "-p", SIDECAR_CRATE, "--target", triple];
if (!isDebug) cargoArgs.push("--release");

console.log(`[stage-sidecar] cargo ${cargoArgs.join(" ")}`);
execFileSync("cargo", cargoArgs, { cwd: repoRoot, stdio: "inherit" });

const built = join(repoRoot, "target", triple, profile, `${BIN_NAME}${ext}`);
const binariesDir = join(repoRoot, "apps", "desktop", "src-tauri", "binaries");
const staged = join(binariesDir, `${BIN_NAME}-${triple}${ext}`);

mkdirSync(binariesDir, { recursive: true });
copyFileSync(built, staged);
console.log(`[stage-sidecar] staged ${built} -> ${staged}`);
