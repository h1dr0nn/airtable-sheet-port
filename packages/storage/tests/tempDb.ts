import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

export type TempDb = {
  path: string;
  cleanup(): void;
};

/** One isolated temp DB file per test; cleanup removes the WAL/SHM files too. */
export function makeTempDb(): TempDb {
  const dir = mkdtempSync(join(tmpdir(), "sheet-port-test-"));
  return {
    path: join(dir, "test.db"),
    cleanup(): void {
      rmSync(dir, { recursive: true, force: true });
    }
  };
}
