// Protocol-level E2E smoke: spawns the Rust MCP sidecar, speaks JSON-RPC over
// stdio, and verifies the v2 tool contract end to end: the tool list, direct
// writes that return a diff, dryRun staging + commit_change, ranged read_cells,
// and audit events.
//
// PREREQUISITE: the debug sidecar must be built WITH the mock connector:
//   cargo build -p sheet-port-mcp --features mock
// `pnpm test:e2e` runs that build first. This script executes the debug binary
// and fails fast when it is missing.
import { spawn } from "node:child_process";
import { DatabaseSync } from "node:sqlite";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const binaryName = process.platform === "win32" ? "sheet-port-mcp.exe" : "sheet-port-mcp";
const serverBinary = join(scriptDir, "..", "target", "debug", binaryName);
if (!existsSync(serverBinary)) {
  process.stderr.write(
    `e2e-smoke: missing sidecar binary at ${serverBinary}\n` +
      "run cargo build -p sheet-port-mcp --features mock first\n"
  );
  process.exit(1);
}

const dbPath = join(tmpdir(), `sheet-port-e2e-${process.pid}.db`);

// Seed v2 leaves fresh databases empty, so the smoke installs its own test
// workspace before spawning the sidecar: the same schema + seed SQL the core
// crate embeds, then the mock source, the Customers table with 3 records,
// and a read+write (no delete) rule on the table.
const sqlDir = join(scriptDir, "..", "crates", "sheet-port-core", "sql");
const nowIso = new Date().toISOString();
{
  const db = new DatabaseSync(dbPath);
  db.exec(readFileSync(join(sqlDir, "schema.sql"), "utf8"));
  db.exec(readFileSync(join(sqlDir, "seed.sql"), "utf8"));
  db.prepare(
    "INSERT INTO sources (id, kind, name, status) VALUES ('mock-source', 'mock', 'Test Workspace', 'connected')"
  ).run();
  db.prepare(
    "INSERT INTO mock_tables (source_id, table_id, name, fields) VALUES ('mock-source', 'customers', 'Customers', ?)"
  ).run(JSON.stringify([
    { name: "Name", type: "string", required: true },
    { name: "Email", type: "email" },
    { name: "Plan", type: "enum", enumValues: ["free", "pro", "enterprise"] },
    { name: "Seats", type: "number" },
    { name: "Active", type: "boolean" }
  ]));
  const insertRecord = db.prepare(
    "INSERT INTO mock_records (source_id, table_id, record_id, fields, position) VALUES ('mock-source', 'customers', ?, ?, ?)"
  );
  const records = [
    ["rec_seed_1", { Name: "Aurora Labs", Email: "ops@auroralabs.dev", Plan: "pro", Seats: 24, Active: true }],
    ["rec_seed_2", { Name: "Basalt Co", Email: "it@basalt.co", Plan: "free", Seats: 3, Active: true }],
    ["rec_seed_3", { Name: "Cirrus Retail", Email: "admin@cirrus.shop", Plan: "enterprise", Seats: 180, Active: false }]
  ];
  records.forEach(([recordId, fields], index) => {
    insertRecord.run(recordId, JSON.stringify(fields), index + 1);
  });
  db.prepare(
    "INSERT INTO permission_rules (source_id, table_id, can_read, can_write, can_delete, updated_at) " +
      "VALUES ('mock-source', 'customers', 1, 1, 0, ?)"
  ).run(nowIso);
  db.close();
}

const child = spawn(serverBinary, [], {
  env: { ...process.env, SHEET_PORT_DB: dbPath },
  stdio: ["pipe", "pipe", "pipe"]
});
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
    }, 15000);
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}
function notify(method, params) {
  child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method, params }) + "\n");
}
function callTool(name, args) {
  return rpc("tools/call", { name, arguments: args ?? {} });
}
function toolJson(res) {
  assert.ok(res.result, `tool result missing: ${JSON.stringify(res).slice(0, 300)}`);
  const text = res.result.content?.[0]?.text ?? "";
  return { isError: res.result.isError === true, text, json: safeParse(text) };
}
function safeParse(text) {
  try { return JSON.parse(text); } catch { return undefined; }
}

try {
  const init = await rpc("initialize", {
    protocolVersion: "2024-11-05",
    capabilities: {},
    clientInfo: { name: "smoke", version: "0.0.0" }
  });
  assert.ok(init.result?.serverInfo, "initialize returned serverInfo");
  const instructions = init.result.instructions ?? "";
  assert.match(instructions, /dryRun/, "instructions explain dryRun staging");
  assert.doesNotMatch(instructions, /approv/i, "no desktop approval wording");
  notify("notifications/initialized");

  const tools = await rpc("tools/list", {});
  const names = tools.result.tools.map((t) => t.name).sort();
  assert.deepEqual(names, [
    "append_records", "commit_change", "create_sheet", "create_spreadsheet",
    "delete_sheet", "describe_table", "find_records", "format_table",
    "get_audit_log", "get_table_style", "list_sheets", "list_sources",
    "list_tables", "read_cells", "read_formulas", "read_table",
    "update_cells", "update_records"
  ], "exactly the 18 contract tools");
  for (const tool of tools.result.tools) {
    assert.doesNotMatch(tool.description, /approv/i, `${tool.name} has no approval wording`);
    const required = tool.inputSchema?.required ?? [];
    assert.ok(!required.includes("sourceId"), `${tool.name} keeps sourceId optional`);
  }

  const src = toolJson(await callTool("list_sources"));
  assert.ok(!src.isError && JSON.stringify(src.json).includes("mock-source"), "list_sources works");

  // Default write: applied immediately, returning the staged diff and the outcome.
  const update = toolJson(await callTool("update_records", {
    sourceId: "mock-source", tableId: "customers",
    patches: [{ recordId: "rec_seed_1", fields: { Seats: 25 } }]
  }));
  assert.ok(!update.isError, `update_records failed: ${update.text}`);
  assert.equal(update.json.committed, true, "writes apply immediately");
  assert.ok(!("payload" in update.json.change), "payload hidden from agent");
  assert.equal(update.json.change.diff[0].before.Seats, 24, "diff shows the before value");
  assert.equal(update.json.change.diff[0].after.Seats, 25, "diff shows the after value");
  assert.equal(update.json.outcome.change.status, "committed");

  const table = toolJson(await callTool("read_table", {
    sourceId: "mock-source", tableId: "customers", limit: 10
  }));
  const seed1 = table.json.records.find((r) => r.id === "rec_seed_1");
  assert.equal(seed1.fields.Seats, 25, "committed patch visible in table data");

  // dryRun: staged only, then applied with commit_change.
  const staged = toolJson(await callTool("update_cells", {
    sourceId: "mock-source", tableId: "customers",
    cells: [{ cell: "B3", value: "smoke@basalt.co" }], dryRun: true
  }));
  assert.ok(!staged.isError, `dry run failed: ${staged.text}`);
  assert.equal(staged.json.committed, false);
  assert.equal(staged.json.change.status, "pending");
  assert.ok(!("outcome" in staged.json), "a dry run has no outcome");
  const changeId = staged.json.change.id;

  const committed = toolJson(await callTool("commit_change", { changeId }));
  assert.ok(!committed.isError, `commit failed: ${committed.text}`);
  assert.equal(committed.json.change.status, "committed");

  const again = toolJson(await callTool("commit_change", { changeId }));
  assert.ok(again.isError, "double commit rejected");

  // read_cells with a range returns only that window, with real row numbers.
  const cells = toolJson(await callTool("read_cells", {
    sourceId: "mock-source", tableId: "customers", range: "B3:C4"
  }));
  assert.ok(!cells.isError, `read_cells failed: ${cells.text}`);
  assert.deepEqual(cells.json.columns, ["B", "C"]);
  assert.equal(cells.json.rows[0].row, 3);
  assert.equal(cells.json.rows[0].cells.B, "smoke@basalt.co", "committed cell write visible");

  const qualified = toolJson(await callTool("read_cells", {
    sourceId: "mock-source", tableId: "customers", range: "Sheet1!A1:B2"
  }));
  assert.ok(qualified.isError, "a sheet-qualified range is rejected");

  const unconfirmed = toolJson(await callTool("delete_sheet", {
    sourceId: "mock-source", tableId: "customers"
  }));
  assert.ok(unconfirmed.isError, "delete_sheet without confirm is rejected");
  assert.match(unconfirmed.text, /confirm: true/);

  const audit = toolJson(await callTool("get_audit_log", { limit: 50 }));
  const actions = audit.json.events.map((e) => e.action);
  for (const action of ["update_records", "update_cells", "commit_change", "read_cells", "get_audit_log"]) {
    assert.ok(actions.includes(action), `audit has ${action}`);
  }
  const dryAudit = audit.json.events.find((e) => e.action === "update_cells");
  assert.equal(dryAudit.metadata.dryRun, true, "audit metadata carries dryRun");

  const hb = new DatabaseSync(dbPath);
  const rows = hb.prepare("SELECT pid, last_seen FROM mcp_heartbeat").all();
  hb.close();
  assert.equal(rows.length, 1, "heartbeat row present");
  assert.equal(Number(rows[0].pid), child.pid, "heartbeat pid matches sidecar");

  process.stdout.write("PROTOCOL SMOKE: ALL PASS\n");
} catch (error) {
  const sidecarLog = Buffer.concat(stderrChunks).toString("utf8");
  if (sidecarLog.length > 0) {
    process.stderr.write(`--- sidecar stderr ---\n${sidecarLog}\n----------------------\n`);
  }
  throw error;
} finally {
  child.kill();
  await new Promise((r) => setTimeout(r, 300));
  try { rmSync(dbPath); rmSync(dbPath + "-wal", { force: true }); rmSync(dbPath + "-shm", { force: true }); } catch {}
}
