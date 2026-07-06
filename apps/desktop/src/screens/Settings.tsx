import { Card, CardContent, CardHeader, CardTitle, Skeleton } from "@sheet-port/ui";
import { useAppStatus } from "../hooks/useAppStatus.js";
import { useTheme } from "../hooks/useTheme.js";
import type { ThemeSetting } from "../lib/theme.js";
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

export function Settings() {
  const { setting, resolved, setSetting } = useTheme();
  const { data: status, isPending } = useAppStatus();

  return (
    <>
      <ScreenHeader title="Settings" description="Appearance and application details" />

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
