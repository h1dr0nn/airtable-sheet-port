import process from "node:process";

/** stdout carries the MCP stdio transport; every log line must go to stderr. */
export function logToStderr(message: string): void {
  process.stderr.write(`[sheet-port-mcp] ${new Date().toISOString()} ${message}\n`);
}

export function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
