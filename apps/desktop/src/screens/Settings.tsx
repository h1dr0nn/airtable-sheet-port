import { useState } from "react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Skeleton
} from "@sheet-port/ui";
import { useAppStatus } from "../hooks/useAppStatus.js";
import {
  useGoogleConfig,
  useGoogleDisconnect,
  useSetGoogleClientId
} from "../hooks/useGoogleConfig.js";
import { useTheme } from "../hooks/useTheme.js";
import type { ThemeSetting } from "../lib/theme.js";
import { ConfirmDialog } from "../components/ConfirmDialog.js";
import { CopyButton } from "../components/CopyButton.js";
import { ScreenHeader } from "../components/ScreenHeader.js";
import { SegmentedControl, type SegmentedOption } from "../components/SegmentedControl.js";

const THEME_OPTIONS: ReadonlyArray<SegmentedOption<ThemeSetting>> = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" }
];

const RESOLVED_LABELS: Record<"light" | "dark", string> = {
  light: "light",
  dark: "dark"
};

function GoogleSheetsCard() {
  const { data: config, isPending } = useGoogleConfig();
  const saveClientId = useSetGoogleClientId();
  const disconnect = useGoogleDisconnect();
  // null draft = "not edited yet"; the input then mirrors the stored value.
  const [draft, setDraft] = useState<string | null>(null);
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  const storedClientId = config?.clientId ?? "";
  const value = draft ?? storedClientId;
  const trimmed = value.trim();
  const canSave = trimmed !== "" && !saveClientId.isPending;
  const connectedEmail = config?.connectedEmail ?? null;

  const save = () => {
    saveClientId.mutate(trimmed, { onSuccess: () => setDraft(null) });
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Google Sheets</CardTitle>
      </CardHeader>
      <CardContent>
        {isPending || !config ? (
          <Skeleton className="h-24" />
        ) : (
          <div className="space-y-4">
            <div className="space-y-1.5">
              <label className="text-[12px] font-medium text-ink-muted" htmlFor="google-client-id">
                OAuth client ID
              </label>
              <div className="flex items-center gap-2">
                <Input
                  id="google-client-id"
                  className="font-mono text-[12.5px]"
                  value={value}
                  placeholder="1234567890-abc.apps.googleusercontent.com"
                  spellCheck={false}
                  autoComplete="off"
                  onChange={(event) => setDraft(event.target.value)}
                />
                <Button size="sm" disabled={!canSave} onClick={save}>
                  {saveClientId.isPending ? "Saving..." : "Save"}
                </Button>
              </div>
              <p className="text-[12px] leading-4 text-ink-muted">
                Desktop-app client ID from Google Cloud Console; no client secret is stored.
              </p>
            </div>

            <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 border-t border-edge pt-4">
              <div className="flex min-w-0 items-center gap-2">
                <Badge variant={connectedEmail ? "success" : "muted"}>
                  {connectedEmail ? "Connected" : "Not connected"}
                </Badge>
                {connectedEmail ? (
                  <span className="truncate font-mono text-[12.5px] text-ink">{connectedEmail}</span>
                ) : (
                  <span className="text-[12.5px] text-ink-muted">
                    Connect from the Data Sources screen
                  </span>
                )}
              </div>
              {connectedEmail ? (
                <Button
                  variant="outline"
                  size="sm"
                  disabled={disconnect.isPending}
                  onClick={() => setIsConfirmOpen(true)}
                >
                  Disconnect
                </Button>
              ) : null}
            </div>
          </div>
        )}
      </CardContent>
      <ConfirmDialog
        open={isConfirmOpen}
        onOpenChange={setIsConfirmOpen}
        title="Disconnect Google Sheets?"
        description="Agents lose access to this account's spreadsheets and the stored token is removed from the OS keychain. You can reconnect at any time."
        confirmLabel="Disconnect"
        isPending={disconnect.isPending}
        onConfirm={() => disconnect.mutate(undefined, { onSettled: () => setIsConfirmOpen(false) })}
      />
    </Card>
  );
}

export function Settings() {
  const { setting, resolved, setSetting } = useTheme();
  const { data: status, isPending } = useAppStatus();

  return (
    <>
      <ScreenHeader title="Settings" description="Appearance, connections, and application details" />

      <div className="max-w-2xl space-y-4">
        <Card>
          <CardHeader>
            <CardTitle>Appearance</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
              <div className="min-w-0">
                <p className="text-[13px] font-medium text-ink">Theme</p>
                <p className="mt-0.5 text-[12.5px] text-ink-muted">
                  {setting === "system"
                    ? `Follows your system preference (currently ${RESOLVED_LABELS[resolved]})`
                    : "Fixed for this device"}
                </p>
              </div>
              <SegmentedControl
                options={THEME_OPTIONS}
                value={setting}
                onChange={setSetting}
                ariaLabel="Theme"
              />
            </div>
          </CardContent>
        </Card>

        <GoogleSheetsCard />

        <Card>
          <CardHeader>
            <CardTitle>About</CardTitle>
          </CardHeader>
          <CardContent className="py-1">
            {isPending || !status ? (
              <Skeleton className="my-3 h-16" />
            ) : (
              <dl className="divide-y divide-edge">
                <div className="flex h-9 items-center justify-between gap-4">
                  <dt className="text-[13px] text-ink-muted">Version</dt>
                  <dd className="font-mono text-[12.5px] text-ink">{status.appVersion}</dd>
                </div>
                <div className="flex h-9 items-center justify-between gap-4">
                  <dt className="shrink-0 text-[13px] text-ink-muted">Database</dt>
                  <dd className="flex min-w-0 items-center gap-1">
                    <span className="truncate font-mono text-[12.5px] text-ink" title={status.dbPath}>
                      {status.dbPath}
                    </span>
                    <CopyButton value={status.dbPath} label="Copy database path" />
                  </dd>
                </div>
              </dl>
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
