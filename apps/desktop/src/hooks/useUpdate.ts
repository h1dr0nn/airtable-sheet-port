import { useCallback, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getErrorMessage } from "../lib/errors.js";
import { isTauri } from "../lib/ipc.js";

/**
 * Self-update state machine backed by @tauri-apps/plugin-updater.
 *
 * The updater endpoint is a static updater.json on GitHub (see tauri.conf.json).
 * `check()` compares the running version against it; when a newer signed bundle
 * exists it returns an Update handle whose `downloadAndInstall()` we drive here.
 *
 * In a plain browser (no Tauri) every action is a no-op that reports
 * `available: false`, so the UI can render the same components in the demo build.
 */

/**
 * The seed updater.json ships with empty platforms, so check() throws
 * "None of the fallback platforms were found". A misconfigured or
 * platform-less manifest is not a real failure for the user - it just means
 * there is nothing to install - so we treat these as "up to date" instead of
 * surfacing a red error. Only genuine network/signature failures remain errors.
 */
function isNoUpdateManifestError(message: string): boolean {
  const normalized = message.toLowerCase();
  return (
    normalized.includes("fallback platforms") ||
    normalized.includes("platform") ||
    normalized.includes("manifest")
  );
}

/**
 * Outcome of a single check(), returned synchronously to the caller so it can
 * react without waiting for a re-render (React state updates are async). The
 * launch check ignores this; the manual button branches on it.
 */
export type UpdateCheckResult =
  | { status: "available"; version: string | null; notes: string | null }
  | { status: "up-to-date" }
  | { status: "error"; message: string };

export type UpdateState = {
  /** A check() call is in flight. */
  checking: boolean;
  /** A newer version was found and is ready to install. */
  available: boolean;
  /** The available version string (e.g. "0.1.0"), null until one is found. */
  version: string | null;
  /** Optional release notes from the manifest. */
  notes: string | null;
  /** downloadAndInstall() is running. */
  downloading: boolean;
  /** Last error message from check()/install(), null when clear. */
  error: string | null;
  /**
   * Runs an update check. Silent by default (no side effects beyond state);
   * callers decide whether to surface "no update" feedback. Returns the outcome
   * synchronously so callers need not wait for the async state to settle.
   */
  check: () => Promise<UpdateCheckResult>;
  /** Downloads and installs the pending update, then relaunches the app. */
  install: () => Promise<void>;
};

export function useUpdate(): UpdateState {
  const [checking, setChecking] = useState(false);
  const [available, setAvailable] = useState(false);
  const [version, setVersion] = useState<string | null>(null);
  const [notes, setNotes] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Hold the Update handle between check() and install() without re-rendering.
  const updateRef = useRef<Update | null>(null);

  const runCheck = useCallback(async (): Promise<UpdateCheckResult> => {
    if (!isTauri) {
      // Browser demo: nothing to update.
      setAvailable(false);
      setVersion(null);
      setNotes(null);
      return { status: "up-to-date" };
    }
    setChecking(true);
    setError(null);
    try {
      const update = await check();
      updateRef.current = update;
      if (update) {
        setAvailable(true);
        setVersion(update.version);
        setNotes(update.body ?? null);
        return { status: "available", version: update.version, notes: update.body ?? null };
      }
      setAvailable(false);
      setVersion(null);
      setNotes(null);
      return { status: "up-to-date" };
    } catch (err: unknown) {
      const message = getErrorMessage(err);
      if (isNoUpdateManifestError(message)) {
        // A platform-less/misconfigured manifest means "nothing to install",
        // not a failure to report. Present it as up to date.
        setAvailable(false);
        setVersion(null);
        setNotes(null);
        setError(null);
        return { status: "up-to-date" };
      }
      setError(message);
      return { status: "error", message };
    } finally {
      setChecking(false);
    }
  }, []);

  const install = useCallback(async () => {
    const update = updateRef.current;
    if (!isTauri || !update) {
      return;
    }
    setDownloading(true);
    setError(null);
    try {
      await update.downloadAndInstall();
      // A successful install requires a relaunch to run the new binary.
      await relaunch();
    } catch (err: unknown) {
      setError(getErrorMessage(err));
      setDownloading(false);
    }
  }, []);

  return {
    checking,
    available,
    version,
    notes,
    downloading,
    error,
    check: runCheck,
    install
  };
}
