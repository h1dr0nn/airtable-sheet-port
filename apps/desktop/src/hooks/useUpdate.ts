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
   * callers decide whether to surface "no update" feedback.
   */
  check: () => Promise<void>;
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

  const runCheck = useCallback(async () => {
    if (!isTauri) {
      // Browser demo: nothing to update.
      setAvailable(false);
      setVersion(null);
      setNotes(null);
      return;
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
      } else {
        setAvailable(false);
        setVersion(null);
        setNotes(null);
      }
    } catch (err: unknown) {
      setError(getErrorMessage(err));
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
