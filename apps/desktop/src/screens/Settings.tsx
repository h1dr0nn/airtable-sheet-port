import { useState, type ReactNode } from "react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Skeleton,
  Tooltip,
  TooltipContent,
  TooltipTrigger
} from "@sheet-port/ui";
import { useAppStatus } from "../hooks/useAppStatus.js";
import {
  useGoogleAccounts,
  useGoogleConfig,
  useSetGoogleClientId,
  useSetGoogleClientSecret
} from "../hooks/useGoogleConfig.js";
import { usePermissionRules } from "../hooks/usePermissions.js";
import {
  useResetSettings,
  useSetFontFamily,
  useSetFontScale,
  useSettings
} from "../hooks/useSettings.js";
import { useSources } from "../hooks/useSources.js";
import { useTheme } from "../hooks/useTheme.js";
import { APP_AUTHOR, APP_NAME } from "../lib/constants.js";
import type { FontFamily, FontScale } from "../lib/ipc.js";
import type { ThemeSetting } from "../lib/theme.js";
import { ConfirmDialog } from "../components/ConfirmDialog.js";
import { GoogleJsonImport } from "../components/settings/GoogleJsonImport.js";
import { CheckForUpdatesButton } from "../components/settings/CheckForUpdatesButton.js";
import { PermissionPresetRow } from "../components/permissions/PermissionPresetRow.js";
import { ScreenHeader } from "../components/ScreenHeader.js";
import { SegmentedControl, type SegmentedOption } from "../components/SegmentedControl.js";
import { McpServerCard } from "../components/settings/McpServerCard.js";
import { McpClientsCard } from "../components/settings/McpClientsCard.js";

const THEME_OPTIONS: ReadonlyArray<SegmentedOption<ThemeSetting>> = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" }
];

const FONT_SCALE_OPTIONS: ReadonlyArray<SegmentedOption<FontScale>> = [
  { value: "small", label: "Small" },
  { value: "normal", label: "Normal" },
  { value: "large", label: "Large" }
];

const FONT_FAMILY_OPTIONS: ReadonlyArray<SegmentedOption<FontFamily>> = [
  { value: "classic", label: "Classic" },
  { value: "modern", label: "Modern" },
  { value: "system", label: "System" }
];

const RESOLVED_LABELS: Record<"light" | "dark", string> = {
  light: "light",
  dark: "dark"
};

type SaveButtonProps = {
  canSave: boolean;
  isPending: boolean;
  onClick: () => void;
  /** Tooltip shown when the button is disabled because nothing changed. */
  disabledReason: string;
};

/** Save button that explains via tooltip why it is disabled (nothing to save).
 * While pending it stays plain since the "Saving..." label is self-explanatory. */
function SaveButton({ canSave, isPending, onClick, disabledReason }: SaveButtonProps) {
  const button = (
    <Button size="sm" disabled={!canSave} onClick={onClick}>
      {isPending ? "Saving..." : "Save"}
    </Button>
  );

  if (canSave || isPending) {
    return button;
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        {/* Span wrapper so the tooltip still fires over a disabled button. */}
        <span className="inline-flex">{button}</span>
      </TooltipTrigger>
      <TooltipContent>{disabledReason}</TooltipContent>
    </Tooltip>
  );
}

/** One labelled Appearance row: title + description on the left, control right. */
function AppearanceRow({
  title,
  description,
  control
}: {
  title: string;
  description: string;
  control: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
      <div className="min-w-0">
        <p className="text-[13px] font-medium text-ink">{title}</p>
        <p className="mt-0.5 text-[12.5px] leading-4 text-ink-muted">{description}</p>
      </div>
      {control}
    </div>
  );
}

function AppearanceCard() {
  const { setting, resolved, setSetting } = useTheme();
  const { data: settings } = useSettings();
  const setFontScale = useSetFontScale();
  const setFontFamily = useSetFontFamily();

  const fontScale = settings?.fontScale ?? "normal";
  const fontFamily = settings?.fontFamily ?? "modern";

  return (
    <Card>
      <CardHeader>
        <CardTitle>Appearance</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="divide-y divide-edge">
          <div className="pb-4 first:pt-0">
            <AppearanceRow
              title="Theme"
              description={
                setting === "system"
                  ? `Follows your system preference (currently ${RESOLVED_LABELS[resolved]})`
                  : "Fixed for this device"
              }
              control={
                <SegmentedControl
                  options={THEME_OPTIONS}
                  value={setting}
                  onChange={setSetting}
                  ariaLabel="Theme"
                />
              }
            />
          </div>
          <div className="py-4">
            <AppearanceRow
              title="Font Size"
              description="Scales the whole interface up or down."
              control={
                <SegmentedControl
                  options={FONT_SCALE_OPTIONS}
                  value={fontScale}
                  onChange={(next) => setFontScale.mutate(next)}
                  ariaLabel="Font Size"
                />
              }
            />
          </div>
          <div className="pt-4 last:pb-0">
            <AppearanceRow
              title="Font"
              description="Classic is a serif face, Modern is Inter, System uses your OS UI font."
              control={
                <SegmentedControl
                  options={FONT_FAMILY_OPTIONS}
                  value={fontFamily}
                  onChange={(next) => setFontFamily.mutate(next)}
                  ariaLabel="Font"
                />
              }
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function ClientIdField({ storedClientId }: { storedClientId: string }) {
  const saveClientId = useSetGoogleClientId();
  // null draft = "not edited yet"; the input then mirrors the stored value.
  const [draft, setDraft] = useState<string | null>(null);

  const value = draft ?? storedClientId;
  const trimmed = value.trim();
  // Dirty check: nothing to save when empty or unchanged.
  const canSave = trimmed !== "" && trimmed !== storedClientId && !saveClientId.isPending;

  const save = () => {
    saveClientId.mutate(trimmed, { onSuccess: () => setDraft(null) });
  };

  return (
    <div className="space-y-1.5">
      <label className="text-[12px] font-medium text-ink-muted" htmlFor="google-client-id">
        OAuth Client ID
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
        <SaveButton
          canSave={canSave}
          isPending={saveClientId.isPending}
          onClick={save}
          disabledReason="No changes to save"
        />
      </div>
      <p className="text-[12px] leading-4 text-ink-muted">
        Desktop-app client ID from Google Cloud Console.
      </p>
    </div>
  );
}

function ClientSecretField({ hasClientSecret }: { hasClientSecret: boolean }) {
  const saveSecret = useSetGoogleClientSecret();
  const [draft, setDraft] = useState("");
  // Replace flow: a stored secret stays masked until the user opts to swap it.
  const [isReplacing, setIsReplacing] = useState(false);
  const [isClearConfirmOpen, setIsClearConfirmOpen] = useState(false);

  const showInput = !hasClientSecret || isReplacing;
  const trimmed = draft.trim();
  // Dirty check: presence only, the stored value can never be compared.
  const canSave = trimmed !== "" && !saveSecret.isPending;

  const save = () => {
    saveSecret.mutate(trimmed, {
      onSuccess: () => {
        setDraft("");
        setIsReplacing(false);
      }
    });
  };

  const clear = () => {
    saveSecret.mutate("", {
      onSettled: () => setIsClearConfirmOpen(false),
      onSuccess: () => {
        setDraft("");
        setIsReplacing(false);
      }
    });
  };

  return (
    <div className="space-y-1.5">
      <label className="text-[12px] font-medium text-ink-muted" htmlFor="google-client-secret">
        OAuth Client Secret
      </label>
      {showInput ? (
        <div className="flex items-center gap-2">
          <Input
            id="google-client-secret"
            type="password"
            className="font-mono text-[12.5px]"
            value={draft}
            placeholder="GOCSPX-..."
            spellCheck={false}
            autoComplete="off"
            onChange={(event) => setDraft(event.target.value)}
          />
          <SaveButton
            canSave={canSave}
            isPending={saveSecret.isPending}
            onClick={save}
            disabledReason="No changes to save"
          />
          {isReplacing ? (
            <Button
              variant="outline"
              size="sm"
              disabled={saveSecret.isPending}
              onClick={() => {
                setDraft("");
                setIsReplacing(false);
              }}
            >
              Cancel
            </Button>
          ) : null}
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <span className="flex-1 truncate font-mono text-[12.5px] text-ink-muted">
            •••••••• Stored in OS keychain
          </span>
          <Button variant="outline" size="sm" onClick={() => setIsReplacing(true)}>
            Replace
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={saveSecret.isPending}
            onClick={() => setIsClearConfirmOpen(true)}
          >
            Clear
          </Button>
        </div>
      )}
      <p className="text-[12px] leading-4 text-ink-muted">
        Google requires the Desktop-app client secret when exchanging the sign-in code; it never
        leaves the keychain.
      </p>
      <ConfirmDialog
        open={isClearConfirmOpen}
        onOpenChange={setIsClearConfirmOpen}
        title="Clear Client Secret?"
        description="The client secret is removed from the OS keychain. Google sign-in will fail until a new secret is saved."
        confirmLabel="Clear"
        isPending={saveSecret.isPending}
        onConfirm={clear}
      />
    </div>
  );
}

function GoogleSheetsCard() {
  const { data: config, isPending } = useGoogleConfig();
  const { data: accounts } = useGoogleAccounts();

  const accountCount = accounts?.length ?? 0;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Google Sheets</CardTitle>
        <GoogleJsonImport />
      </CardHeader>
      <CardContent>
        {isPending || !config ? (
          <Skeleton className="h-24" />
        ) : (
          <div className="space-y-4">
            <ClientIdField storedClientId={config.clientId ?? ""} />
            <ClientSecretField hasClientSecret={config.hasClientSecret} />

            <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 border-t border-edge pt-4">
              <div className="flex min-w-0 items-center gap-2">
                <Badge variant={accountCount > 0 ? "success" : "muted"}>
                  {accountCount > 0 ? "Connected" : "Not connected"}
                </Badge>
                <span className="text-[12.5px] text-ink-muted">
                  {accountCount > 0
                    ? `${accountCount} account${accountCount === 1 ? "" : "s"} linked. Manage them in Data Sources.`
                    : "Connect an account from the Data Sources screen"}
                </span>
              </div>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

/** One preset-driven source-wide rule per connected source. Auto-approve is a
 * global backend setting, so choosing Auto Approve / Bypass on any source turns
 * it on app-wide; the hint below calls this out. */
function PermissionsCard() {
  const { data: sources, isPending: isSourcesPending } = useSources();
  const { data: rules, isPending: isRulesPending } = usePermissionRules();
  const { data: settings, isPending: isSettingsPending } = useSettings();
  const sourceList = sources ?? [];
  const ruleList = rules ?? [];
  const autoApproveWrites = settings?.autoApproveWrites ?? false;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Permissions</CardTitle>
      </CardHeader>
      <CardContent>
        {isSourcesPending || isRulesPending || isSettingsPending ? (
          <Skeleton className="h-24" />
        ) : sourceList.length === 0 ? (
          <p className="rounded-md border border-edge bg-surface px-3 py-6 text-center text-[12.5px] text-ink-muted">
            Connect a data source first
          </p>
        ) : (
          <>
            <p className="mb-1 text-[12px] leading-4 text-ink-muted">
              Pick an access preset per source. Auto Approve and Bypass turn on global
              auto-approve, which applies to every connected source.
            </p>
            <div className="divide-y divide-edge">
              {sourceList.map((source) => (
                <PermissionPresetRow
                  key={source.id}
                  source={source}
                  autoApproveWrites={autoApproveWrites}
                  rule={ruleList.find(
                    (rule) => rule.sourceId === source.id && rule.tableId === null
                  )}
                />
              ))}
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}

function AboutCard() {
  const { data: status, isPending } = useAppStatus();

  return (
    <Card>
      <CardHeader>
        <CardTitle>About</CardTitle>
        <CheckForUpdatesButton />
      </CardHeader>
      <CardContent className="py-1">
        {isPending || !status ? (
          <Skeleton className="my-3 h-16" />
        ) : (
          <dl className="divide-y divide-edge">
            <div className="flex h-9 items-center justify-between gap-4">
              <dt className="text-[13px] text-ink-muted">App Name</dt>
              <dd className="text-[12.5px] font-medium text-ink">{APP_NAME}</dd>
            </div>
            <div className="flex h-9 items-center justify-between gap-4">
              <dt className="text-[13px] text-ink-muted">Version</dt>
              <dd className="font-mono text-[12.5px] text-ink">{status.appVersion}</dd>
            </div>
            <div className="flex h-9 items-center justify-between gap-4">
              <dt className="text-[13px] text-ink-muted">Created By</dt>
              <dd className="text-[12.5px] font-medium text-ink">{APP_AUTHOR}</dd>
            </div>
            <div className="flex h-9 items-center justify-between gap-4">
              <dt className="shrink-0 text-[13px] text-ink-muted">Database</dt>
              <Tooltip>
                <TooltipTrigger asChild>
                  <dd className="min-w-0 truncate font-mono text-[12.5px] text-ink-muted">
                    {status.dbPath}
                  </dd>
                </TooltipTrigger>
                <TooltipContent className="max-w-md break-all font-mono">
                  {status.dbPath}
                </TooltipContent>
              </Tooltip>
            </div>
          </dl>
        )}
      </CardContent>
    </Card>
  );
}

/** Resets frontend prefs (theme) and app-managed prefs (auto-approve) only. */
function ResetCard() {
  const resetSettings = useResetSettings();
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Reset</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
          <p className="min-w-0 text-[12.5px] leading-4 text-ink-muted">
            Restore preferences to their defaults. Your Google credentials, permission rules, and
            data are not affected.
          </p>
          <Button
            variant="outline"
            size="sm"
            disabled={resetSettings.isPending}
            onClick={() => setIsConfirmOpen(true)}
          >
            Reset to Default
          </Button>
        </div>
      </CardContent>
      <ConfirmDialog
        open={isConfirmOpen}
        onOpenChange={setIsConfirmOpen}
        title="Reset to Default?"
        description="Theme, font, and auto-approve return to their defaults. This does NOT remove your Google credentials, permission rules, or data."
        confirmLabel="Reset to Default"
        isPending={resetSettings.isPending}
        onConfirm={() => resetSettings.mutate(undefined, { onSettled: () => setIsConfirmOpen(false) })}
      />
    </Card>
  );
}

export function Settings() {
  return (
    <>
      <ScreenHeader
        title="Settings"
        description="Appearance, connections, permissions, and application details"
      />

      <div className="space-y-4">
        <AppearanceCard />
        <McpServerCard />
        <McpClientsCard />
        <GoogleSheetsCard />
        <PermissionsCard />
        <AboutCard />
        <ResetCard />
      </div>
    </>
  );
}
