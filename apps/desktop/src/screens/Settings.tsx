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
  Switch,
  Tooltip,
  TooltipContent,
  TooltipTrigger
} from "@sheet-port/ui";
import { useAppStatus } from "../hooks/useAppStatus.js";
import {
  useAutostartEnabled,
  useSetAutostartEnabled,
  useSetCloseBehavior
} from "../hooks/useCloseBehavior.js";
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
  useSetLanguage,
  useSettings
} from "../hooks/useSettings.js";
import { useSources } from "../hooks/useSources.js";
import { useTheme } from "../hooks/useTheme.js";
import { useTranslation } from "../i18n/useTranslation.js";
import type { TranslationKey } from "../i18n/translations.js";
import { APP_AUTHOR, APP_NAME } from "../lib/constants.js";
import { isTauri, type CloseBehavior, type FontFamily, type FontScale, type Language } from "../lib/ipc.js";
import type { ThemeSetting } from "../lib/theme.js";
import { ConfirmDialog } from "../components/ConfirmDialog.js";
import { GoogleJsonImport } from "../components/settings/GoogleJsonImport.js";
import { CheckForUpdatesButton } from "../components/settings/CheckForUpdatesButton.js";
import { PermissionPresetRow } from "../components/permissions/PermissionPresetRow.js";
import { ScreenHeader } from "../components/ScreenHeader.js";
import { SegmentedControl, type SegmentedOption } from "../components/SegmentedControl.js";
import { McpServerCard } from "../components/settings/McpServerCard.js";
import { McpClientsCard } from "../components/settings/McpClientsCard.js";

// Option value/labelKey tables; labels resolve through t() per language.
const THEME_OPTIONS: ReadonlyArray<{ value: ThemeSetting; labelKey: TranslationKey }> = [
  { value: "light", labelKey: "theme.light" },
  { value: "dark", labelKey: "theme.dark" },
  { value: "system", labelKey: "theme.system" }
];

const FONT_SCALE_OPTIONS: ReadonlyArray<{ value: FontScale; labelKey: TranslationKey }> = [
  { value: "small", labelKey: "settings.appearance.fontSizeSmall" },
  { value: "normal", labelKey: "settings.appearance.fontSizeNormal" },
  { value: "large", labelKey: "settings.appearance.fontSizeLarge" }
];

const FONT_FAMILY_OPTIONS: ReadonlyArray<{ value: FontFamily; labelKey: TranslationKey }> = [
  { value: "classic", labelKey: "settings.appearance.fontClassic" },
  { value: "modern", labelKey: "settings.appearance.fontModern" },
  { value: "system", labelKey: "settings.appearance.fontSystem" }
];

const LANGUAGE_OPTIONS: ReadonlyArray<{ value: Language; labelKey: TranslationKey }> = [
  { value: "en", labelKey: "settings.appearance.languageEnglish" },
  { value: "vi", labelKey: "settings.appearance.languageVietnamese" }
];

const CLOSE_BEHAVIOR_OPTIONS: ReadonlyArray<{ value: CloseBehavior; labelKey: TranslationKey }> = [
  { value: "ask", labelKey: "settings.general.closeAsk" },
  { value: "tray", labelKey: "settings.general.closeTray" },
  { value: "quit", labelKey: "settings.general.closeQuit" }
];

/** Builds SegmentedControl options by resolving each labelKey through t(). */
function toOptions<T extends string>(
  entries: ReadonlyArray<{ value: T; labelKey: TranslationKey }>,
  t: (id: TranslationKey) => string
): ReadonlyArray<SegmentedOption<T>> {
  return entries.map((entry) => ({ value: entry.value, label: t(entry.labelKey) }));
}

const RESOLVED_LABEL_KEYS: Record<"light" | "dark", TranslationKey> = {
  light: "theme.light",
  dark: "theme.dark"
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
  const { t } = useTranslation();
  const button = (
    <Button size="sm" disabled={!canSave} onClick={onClick}>
      {isPending ? t("common.saving") : t("common.save")}
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
  const { t } = useTranslation();
  const setFontScale = useSetFontScale();
  const setFontFamily = useSetFontFamily();
  const setLanguage = useSetLanguage();

  const fontScale = settings?.fontScale ?? "normal";
  const fontFamily = settings?.fontFamily ?? "modern";
  const language = settings?.language ?? "en";

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.appearance.title")}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="divide-y divide-edge">
          <div className="pb-4 first:pt-0">
            <AppearanceRow
              title={t("settings.appearance.theme")}
              description={
                setting === "system"
                  ? t("settings.appearance.themeFollowsSystem", {
                      mode: t(RESOLVED_LABEL_KEYS[resolved])
                    })
                  : t("settings.appearance.themeFixed")
              }
              control={
                <SegmentedControl
                  options={toOptions(THEME_OPTIONS, t)}
                  value={setting}
                  onChange={setSetting}
                  ariaLabel={t("settings.appearance.theme")}
                />
              }
            />
          </div>
          <div className="py-4">
            <AppearanceRow
              title={t("settings.appearance.fontSize")}
              description={t("settings.appearance.fontSizeDescription")}
              control={
                <SegmentedControl
                  options={toOptions(FONT_SCALE_OPTIONS, t)}
                  value={fontScale}
                  onChange={(next) => setFontScale.mutate(next)}
                  ariaLabel={t("settings.appearance.fontSize")}
                />
              }
            />
          </div>
          <div className="py-4">
            <AppearanceRow
              title={t("settings.appearance.font")}
              description={t("settings.appearance.fontDescription")}
              control={
                <SegmentedControl
                  options={toOptions(FONT_FAMILY_OPTIONS, t)}
                  value={fontFamily}
                  onChange={(next) => setFontFamily.mutate(next)}
                  ariaLabel={t("settings.appearance.font")}
                />
              }
            />
          </div>
          <div className="pt-4 last:pb-0">
            <AppearanceRow
              title={t("settings.appearance.language")}
              description={t("settings.appearance.languageDescription")}
              control={
                <SegmentedControl
                  options={toOptions(LANGUAGE_OPTIONS, t)}
                  value={language}
                  onChange={(next) => setLanguage.mutate(next)}
                  ariaLabel={t("settings.appearance.language")}
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
  const { t } = useTranslation();
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
        {t("settings.google.clientId")}
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
          disabledReason={t("common.noChangesToSave")}
        />
      </div>
      <p className="text-[12px] leading-4 text-ink-muted">
        {t("settings.google.clientIdHint")}
      </p>
    </div>
  );
}

function ClientSecretField({ hasClientSecret }: { hasClientSecret: boolean }) {
  const saveSecret = useSetGoogleClientSecret();
  const { t } = useTranslation();
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
        {t("settings.google.clientSecret")}
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
            disabledReason={t("common.noChangesToSave")}
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
              {t("common.cancel")}
            </Button>
          ) : null}
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <span className="flex-1 truncate font-mono text-[12.5px] text-ink-muted">
            {t("settings.google.storedInKeychain")}
          </span>
          <Button variant="outline" size="sm" onClick={() => setIsReplacing(true)}>
            {t("settings.google.replace")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={saveSecret.isPending}
            onClick={() => setIsClearConfirmOpen(true)}
          >
            {t("settings.google.clear")}
          </Button>
        </div>
      )}
      <p className="text-[12px] leading-4 text-ink-muted">
        {t("settings.google.clientSecretHint")}
      </p>
      <ConfirmDialog
        open={isClearConfirmOpen}
        onOpenChange={setIsClearConfirmOpen}
        title={t("settings.google.clearSecretTitle")}
        description={t("settings.google.clearSecretDescription")}
        confirmLabel={t("settings.google.clear")}
        isPending={saveSecret.isPending}
        onConfirm={clear}
      />
    </div>
  );
}

function GoogleSheetsCard() {
  const { data: config, isPending } = useGoogleConfig();
  const { data: accounts } = useGoogleAccounts();
  const { t } = useTranslation();

  const accountCount = accounts?.length ?? 0;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.google.title")}</CardTitle>
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
                  {accountCount > 0 ? t("common.connected") : t("common.notConnected")}
                </Badge>
                <span className="text-[12.5px] text-ink-muted">
                  {accountCount > 0
                    ? t("settings.google.accountsLinked", { count: accountCount })
                    : t("settings.google.connectFromSources")}
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
  const { t } = useTranslation();
  const sourceList = sources ?? [];
  const ruleList = rules ?? [];
  // Auto-approve is on by default; ?? true keeps the loading state consistent.
  const autoApproveWrites = settings?.autoApproveWrites ?? true;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.permissions.title")}</CardTitle>
      </CardHeader>
      <CardContent>
        {isSourcesPending || isRulesPending || isSettingsPending ? (
          <Skeleton className="h-24" />
        ) : sourceList.length === 0 ? (
          <p className="rounded-md border border-edge bg-surface px-3 py-6 text-center text-[12.5px] text-ink-muted">
            {t("settings.permissions.connectFirst")}
          </p>
        ) : (
          <>
            <p className="mb-1 text-[12px] leading-4 text-ink-muted">
              {t("settings.permissions.hint")}
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
  const { t } = useTranslation();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.about.title")}</CardTitle>
        <CheckForUpdatesButton />
      </CardHeader>
      <CardContent className="py-1">
        {isPending || !status ? (
          <Skeleton className="my-3 h-16" />
        ) : (
          <dl className="divide-y divide-edge">
            <div className="flex h-9 items-center justify-between gap-4">
              <dt className="text-[13px] text-ink-muted">{t("settings.about.appName")}</dt>
              <dd className="text-[12.5px] font-medium text-ink">{APP_NAME}</dd>
            </div>
            <div className="flex h-9 items-center justify-between gap-4">
              <dt className="text-[13px] text-ink-muted">{t("settings.about.version")}</dt>
              <dd className="font-mono text-[12.5px] text-ink">{status.appVersion}</dd>
            </div>
            <div className="flex h-9 items-center justify-between gap-4">
              <dt className="text-[13px] text-ink-muted">{t("settings.about.createdBy")}</dt>
              <dd className="text-[12.5px] font-medium text-ink">{APP_AUTHOR}</dd>
            </div>
            <div className="flex h-9 items-center justify-between gap-4">
              <dt className="shrink-0 text-[13px] text-ink-muted">{t("settings.about.database")}</dt>
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

/** Window-behavior preferences: what closing the window does, and whether the
 * app launches at login. Launch at Login needs OS integration, so it is only
 * shown under Tauri; the browser preview hides it. */
function GeneralCard() {
  const { data: settings } = useSettings();
  const { t } = useTranslation();
  const setCloseBehavior = useSetCloseBehavior();
  const { data: autostartEnabled } = useAutostartEnabled();
  const setAutostart = useSetAutostartEnabled();

  const closeBehavior = settings?.closeBehavior ?? "ask";

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.general.title")}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="divide-y divide-edge">
          <div className="pb-4 first:pt-0">
            <AppearanceRow
              title={t("settings.general.whenClosing")}
              description={t("settings.general.whenClosingDescription")}
              control={
                <SegmentedControl
                  options={toOptions(CLOSE_BEHAVIOR_OPTIONS, t)}
                  value={closeBehavior}
                  onChange={(next) => setCloseBehavior.mutate(next)}
                  ariaLabel={t("settings.general.whenClosing")}
                />
              }
            />
          </div>
          {isTauri ? (
            <div className="pt-4 last:pb-0">
              <AppearanceRow
                title={t("settings.general.launchAtLogin")}
                description={t("settings.general.launchAtLoginDescription")}
                control={
                  <Switch
                    checked={autostartEnabled ?? false}
                    onCheckedChange={(checked) => setAutostart.mutate(checked)}
                    disabled={setAutostart.isPending}
                    aria-label={t("settings.general.launchAtLogin")}
                  />
                }
              />
            </div>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}

/** Resets frontend prefs (theme) and app-managed prefs (auto-approve) only. */
function ResetCard() {
  const resetSettings = useResetSettings();
  const { t } = useTranslation();
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.reset.title")}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
          <p className="min-w-0 text-[12.5px] leading-4 text-ink-muted">
            {t("settings.reset.description")}
          </p>
          <Button
            variant="outline"
            size="sm"
            disabled={resetSettings.isPending}
            onClick={() => setIsConfirmOpen(true)}
          >
            {t("settings.reset.button")}
          </Button>
        </div>
      </CardContent>
      <ConfirmDialog
        open={isConfirmOpen}
        onOpenChange={setIsConfirmOpen}
        title={t("settings.reset.confirmTitle")}
        description={t("settings.reset.confirmDescription")}
        confirmLabel={t("settings.reset.button")}
        isPending={resetSettings.isPending}
        onConfirm={() => resetSettings.mutate(undefined, { onSettled: () => setIsConfirmOpen(false) })}
      />
    </Card>
  );
}

export function Settings() {
  const { t } = useTranslation();
  return (
    <>
      <ScreenHeader
        title={t("screen.settings.title")}
        description={t("screen.settings.description")}
      />

      <div className="space-y-4">
        <AppearanceCard />
        <McpServerCard />
        <McpClientsCard />
        <GoogleSheetsCard />
        <PermissionsCard />
        <GeneralCard />
        <AboutCard />
        <ResetCard />
      </div>
    </>
  );
}
