#!/usr/bin/env node
import process from "node:process";
import { clearInterval, setInterval } from "node:timers";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { HEARTBEAT_INTERVAL_MS, HEARTBEAT_STALE_MS } from "@sheet-port/storage";
import { createAppContext } from "./context.js";
import { describeError, logToStderr } from "./logger.js";
import { createServer } from "./tools.js";

const context = createAppContext();
const server = createServer(context);
const transport = new StdioServerTransport();

await server.connect(transport);

// Heartbeat: the desktop app treats the sidecar as running while our row in
// mcp_heartbeat stays fresh. Clean up rows left behind by crashed processes.
context.heartbeat.deleteStale(HEARTBEAT_STALE_MS);
context.heartbeat.upsertOwn(process.pid);
const heartbeatTimer = setInterval(() => {
  try {
    context.heartbeat.upsertOwn(process.pid);
  } catch (error) {
    logToStderr(`heartbeat update failed: ${describeError(error)}`);
  }
}, HEARTBEAT_INTERVAL_MS);
// Never keep the process alive just for the heartbeat.
heartbeatTimer.unref();

let hasShutDown = false;
function shutdown(): void {
  if (hasShutDown) {
    return;
  }
  hasShutDown = true;
  clearInterval(heartbeatTimer);
  try {
    context.heartbeat.deleteOwn(process.pid);
  } catch (error) {
    // Best effort: the DB may already be unavailable while the process exits.
    logToStderr(`heartbeat cleanup failed: ${describeError(error)}`);
  }
}

process.on("SIGINT", () => {
  shutdown();
  process.exit(0);
});
process.on("SIGTERM", () => {
  shutdown();
  process.exit(0);
});
process.on("exit", shutdown);

logToStderr(`ready (pid ${process.pid}, db ${context.dbPath})`);
