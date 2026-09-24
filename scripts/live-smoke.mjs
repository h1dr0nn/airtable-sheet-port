// Live smoke against a REAL Apps Script bridge and Google Sheets. Not part of
// `pnpm test`: it needs network access and a bridge, and it writes to a real
// spreadsheet (inside a temporary tab it creates and deletes again).
//
//   SHEET_PORT_LIVE_BRIDGE_URL     the bridge /exec URL
//   SHEET_PORT_LIVE_BRIDGE_SECRET  the bridge secret
//   SHEET_PORT_LIVE_SPREADSHEET    a spreadsheet URL or id the account can edit
//
// Runs the RELEASE sidecar (no mock): `cargo build --release -p sheet-port-mcp`.
// Uses a throwaway database; the bridge credential is written to the OS
// keychain entry for the bridge's account, exactly like adding it in the app.
import { spawn, spawnSync } from "node:child_process";
import { existsSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const { SHEET_PORT_LIVE_BRIDGE_URL: bridgeUrl, SHEET_PORT_LIVE_BRIDGE_SECRET: secret } = process.env;
const spreadsheet = process.env.SHEET_PORT_LIVE_SPREADSHEET;
if (!bridgeUrl || !secret || !spreadsheet) {
  process.stderr.write(
    "live-smoke: set SHEET_PORT_LIVE_BRIDGE_URL, SHEET_PORT_LIVE_BRIDGE_SECRET and SHEET_PORT_LIVE_SPREADSHEET\n"
  );
  process.exit(1);
}

const scriptDir = dirname(fileURLToPath(import.meta.url));
const binaryName = process.platform === "win32" ? "sheet-port-mcp.exe" : "sheet-port-mcp";
const serverBinary = join(scriptDir, "..", "target", "release", binaryName);
if (!existsSync(serverBinary)) {
  process.stderr.write(`live-smoke: build the release sidecar first (missing ${serverBinary})\n`);
  process.exit(1);
}

const dbPath = join(tmpdir(), `sheet-port-live-${process.pid}.db`);
const env = { ...process.env, SHEET_PORT_DB: dbPath };

// 1. Add the bridge headlessly (secret via env, never argv).
const added = spawnSync(serverBinary, ["bridge", "add", bridgeUrl], {
  env: { ...env, SHEET_PORT_BRIDGE_SECRET: secret },
  encoding: "utf8"
});
assert.equal(added.status, 0, `bridge add failed: ${added.stderr}`);
const account = JSON.parse(added.stdout);
console.log(`bridge ok: ${account.email} (${account.sourceId})`);

// 2. Speak MCP to the release sidecar.
const child = spawn(serverBinary, [], { env, stdio: ["pipe", "pipe", "pipe"] });
const stderrChunks = [];
child.stderr.on("data", (c) => stderrChunks.push(c));
let buffer = "";
const pending = new Map();
child.stdout.on("data", (chunk) => {
  buffer += chunk.toString("utf8");
  let idx;
  while ((idx = buffer.indexOf("\n")) >= 0) {
    const line = buffer.slice(0, idx).trim();
    buffer = buffer.slice(idx + 1);
    if (!line) continue;
    const msg = JSON.parse(line);
    if (msg.id !== undefined && pending.has(msg.id)) {
      pending.get(msg.id)(msg);
      pending.delete(msg.id);
    }
  }
});
let nextId = 1;
function rpc(method, params) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, resolve);
    setTimeout(() => {
      if (pending.delete(id)) reject(new Error(`timeout waiting for ${method}`));
    }, 60000);
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}
/** Calls a tool WITHOUT sourceId (exercises routing) and returns its JSON, failing on tool errors. */
async function tool(name, args) {
  const res = await rpc("tools/call", { name, arguments: args ?? {} });
  const text = res.result?.content?.[0]?.text ?? JSON.stringify(res);
  assert.ok(res.result && res.result.isError !== true, `${name} failed: ${text}`);
  return JSON.parse(text);
}

let tabGid;
try {
  await rpc("initialize", {
    protocolVersion: "2024-11-05",
    capabilities: {},
    clientInfo: { name: "live-smoke", version: "0.0.0" }
  });
  child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");

  const sheets = await tool("list_sheets", { tableId: spreadsheet });
  console.log(`list_sheets ok: ${sheets.sheets.length} tab(s)`);
  const spreadsheetId = sheets.spreadsheetId;

  const tabTitle = `sheet-port-live-${Date.now()}`;
  const created = await tool("create_sheet", { tableId: spreadsheetId, title: tabTitle });
  assert.equal(created.committed, true);
  tabGid = created.outcome.created.sheetGid;
  assert.ok(tabGid !== undefined, `create_sheet returned a gid: ${JSON.stringify(created.outcome.created)}`);
  const tab = `${spreadsheetId}:${tabGid}`;
  console.log(`create_sheet ok: ${tabTitle}`);

  const appended = await tool("append_records", {
    tableId: tab,
    records: [
      { Name: "Alpha", Score: 10 },
      { Name: "Beta", Score: 20 }
    ],
    formats: [{ range: "A1:B1", bold: true, backgroundColor: "#dde7f5" }],
    freezeRows: 1
  });
  assert.equal(appended.committed, true);
  assert.equal(appended.outcome.formatError ?? null, null, "bundled format applied");
  console.log("append_records + format ok");

  const read = await tool("read_table", { tableId: tab });
  assert.deepEqual(read.records.map((r) => r.fields.Name), ["Alpha", "Beta"]);
  console.log("read_table ok");

  const staged = await tool("update_records", {
    tableId: tab,
    patches: [{ recordId: read.records[1].id, fields: { Score: 25 } }],
    dryRun: true
  });
  assert.equal(staged.committed, false);
  await tool("commit_change", { changeId: staged.change.id });
  console.log("update_records dryRun + commit_change ok");

  await tool("update_cells", { tableId: tab, cells: [{ cell: "C1", value: "Total" }, { cell: "C2", value: "=B2+B3" }] });
  const cells = await tool("read_cells", { tableId: tab, range: "A1:C3" });
  const byRow = Object.fromEntries(cells.rows.map((r) => [r.row, r.cells]));
  assert.equal(byRow[3].B, "25", "committed update visible");
  assert.equal(byRow[2].C, "35", "formula evaluated");
  console.log("update_cells + read_cells range ok");

  const style = await tool("get_table_style", { tableId: tab });
  assert.equal(style.style.frozenRowCount, 1, "freeze applied");
  console.log("get_table_style ok");

  const refused = await rpc("tools/call", { name: "delete_sheet", arguments: { tableId: tab } });
  assert.equal(refused.result?.isError, true, "delete_sheet without confirm is refused");
  await tool("delete_sheet", { tableId: tab, confirm: true });
  tabGid = undefined;
  console.log("delete_sheet ok");

  console.log("LIVE SMOKE PASSED");
} catch (error) {
  process.stderr.write(`LIVE SMOKE FAILED: ${error.stack ?? error}\n${Buffer.concat(stderrChunks).toString()}\n`);
  if (tabGid !== undefined) process.stderr.write(`temporary tab gid ${tabGid} was left in the spreadsheet\n`);
  process.exitCode = 1;
} finally {
  // Windows keeps the SQLite files locked until the sidecar has exited.
  // A sidecar that already exited never emits "exit" again.
  const exited =
    child.exitCode === null && child.signalCode === null
      ? new Promise((resolve) => child.once("exit", resolve))
      : Promise.resolve();
  child.stdin.end();
  child.kill();
  await exited;
  for (const suffix of ["", "-wal", "-shm"]) rmSync(dbPath + suffix, { force: true });
}
