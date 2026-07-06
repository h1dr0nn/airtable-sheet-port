// Protocol-level E2E smoke: spawns the built MCP sidecar, speaks JSON-RPC over
// stdio, and verifies the preview -> approve -> commit enforcement end to end.
//
// PREREQUISITE: run "pnpm build" at the repo root first. This script executes
// the compiled sidecar at ../dist/index.js and fails fast when it is missing.
import { spawn } from "node:child_process";
import { DatabaseSync } from "node:sqlite";
import { existsSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const serverEntry = join(scriptDir, "..", "dist", "index.js");
if (!existsSync(serverEntry)) {
  process.stderr.write(
    `e2e-smoke: missing build output at ${serverEntry}\n` +
      'Run "pnpm build" at the repository root first, then re-run "pnpm test:e2e".\n'
  );
  process.exit(1);
}

const dbPath = join(tmpdir(), `sheet-port-e2e-${process.pid}.db`);

const child = spawn(process.execPath, [serverEntry], {
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
  notify("notifications/initialized");

  const tools = await rpc("tools/list", {});
  const names = tools.result.tools.map((t) => t.name).sort();
  assert.deepEqual(names, [
    "append_records", "commit_change", "describe_table", "find_records",
    "get_audit_log", "list_sources", "list_tables", "preview_update_records", "read_table"
  ], "exactly the 9 contract tools");

  const src = toolJson(await callTool("list_sources"));
  assert.ok(!src.isError && JSON.stringify(src.json).includes("mock-source"), "list_sources works");

  const preview = toolJson(await callTool("preview_update_records", {
    sourceId: "mock-source", tableId: "customers",
    patches: [{ recordId: "rec_seed_1", fields: { Seats: 25 } }]
  }));
  assert.ok(!preview.isError, `preview failed: ${preview.text}`);
  const changeId = preview.json.change.id;
  assert.equal(preview.json.requiresConfirmation, true, "seed rule requires confirmation");
  assert.equal(preview.json.change.status, "pending");
  assert.ok(!("payload" in preview.json.change), "payload hidden from agent");

  const blocked = toolJson(await callTool("commit_change", { changeId }));
  assert.ok(blocked.isError, "commit without approval must fail");
  assert.match(blocked.text, /approval|approve/i, "error explains approval is needed");

  // Simulate the desktop app approving the change (same SQL the Rust command runs).
  const db = new DatabaseSync(dbPath);
  const updated = db.prepare(
    "UPDATE pending_changes SET status='approved', decided_at=?, decided_by='user' WHERE id=? AND status='pending'"
  ).run(new Date().toISOString(), changeId);
  assert.equal(updated.changes, 1, "desktop-side approval applied");
  db.close();

  const committed = toolJson(await callTool("commit_change", { changeId }));
  assert.ok(!committed.isError, `commit after approval failed: ${committed.text}`);
  assert.equal(committed.json.change.status, "committed");

  const again = toolJson(await callTool("commit_change", { changeId }));
  assert.ok(again.isError, "double commit rejected");

  const table = toolJson(await callTool("read_table", {
    sourceId: "mock-source", tableId: "customers", limit: 10
  }));
  const seed1 = table.json.records.find((r) => r.id === "rec_seed_1");
  assert.equal(seed1.fields.Seats, 25, "committed patch visible in table data");

  const audit = toolJson(await callTool("get_audit_log", { limit: 50 }));
  const actions = audit.json.events.map((e) => e.action);
  assert.ok(actions.includes("commit_change"), "audit has commit_change");
  assert.ok(actions.includes("get_audit_log"), "get_audit_log self-audited");

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
