import { useEffect } from "react";
import { toast } from "@sheet-port/ui";
import { useAppStatus } from "./useAppStatus.js";
import { useTranslation } from "../i18n/useTranslation.js";
import { isTauri } from "../lib/ipc.js";

/** localStorage key remembering the app version of the previous run. */
const LAST_RUN_VERSION_KEY = "sheet-port:last-run-version";

/**
 * After an update installs and the app relaunches, remind the user that stdio
 * MCP clients (Claude Desktop, Claude Code, ...) keep running the OLD sidecar
 * until they restart - the app cannot force a host to reconnect. Fires once
 * per version change; the very first run only records the version.
 */
export function useUpdateRestartNotice() {
  const { data: status } = useAppStatus();
  const { t } = useTranslation();
  const version = status?.appVersion;

  useEffect(() => {
    if (!isTauri || !version) {
      return;
    }
    const lastRun = window.localStorage.getItem(LAST_RUN_VERSION_KEY);
    if (lastRun && lastRun !== version) {
      toast.info(t("toast.updatedRestartClients", { version }));
    }
    window.localStorage.setItem(LAST_RUN_VERSION_KEY, version);
  }, [version, t]);
}
