import { useState, type ReactNode } from "react";
import { Loader2, Plus } from "lucide-react";
import {
  Badge,
  Button,
  cn,
  FOCUS_RING,
  Skeleton,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  type BadgeVariant
} from "@sheet-port/ui";
import type { DataSource, SourceStatus } from "@sheet-port/shared";
import {
  useGoogleAccounts,
  useGoogleConfig,
  useGoogleConnect,
  useGoogleDisconnect
} from "../hooks/useGoogleConfig.js";
import { useSources } from "../hooks/useSources.js";
import { useTranslation } from "../i18n/useTranslation.js";
import type { TranslationKey } from "../i18n/translations.js";
import type { GoogleAccount } from "../lib/ipc.js";
import type { ScreenId } from "../lib/nav.js";
import { ConfirmDialog } from "../components/ConfirmDialog.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const STATUS_VARIANTS: Record<SourceStatus, BadgeVariant> = {
  connected: "success",
  placeholder: "muted",
  error: "danger"
};

const STATUS_LABEL_KEYS: Record<SourceStatus, TranslationKey> = {
  connected: "sources.statusConnected",
  placeholder: "sources.statusPlaceholder",
  error: "sources.statusError"
};

type SourceCardShellProps = {
  overline: string;
  badge: ReactNode;
  children: ReactNode;
  footer: ReactNode;
};

function SourceCardShell({ overline, badge, children, footer }: SourceCardShellProps) {
  return (
    <article className="flex flex-col rounded-card border border-edge bg-raised shadow-card">
      <header className="flex items-center justify-between gap-3 border-b border-edge px-5 py-3">
        <h3 className="overline-label">{overline}</h3>
        {badge}
      </header>
      <div className="flex-1 px-5 py-4">{children}</div>
      <footer className="flex items-center gap-3 px-5 pb-4">{footer}</footer>
    </article>
  );
}

/** One connected Google account: its email plus a confirmed Disconnect. */
function GoogleAccountCard({ account }: { account: GoogleAccount }) {
  const disconnect = useGoogleDisconnect();
  const { t } = useTranslation();
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  return (
    <>
      <SourceCardShell
        overline="google_sheets"
        badge={<Badge variant="success">{t("common.connected")}</Badge>}
        footer={
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                disabled={disconnect.isPending}
                onClick={() => setIsConfirmOpen(true)}
              >
                {t("sources.disconnect")}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("sources.disconnectTooltip")}</TooltipContent>
          </Tooltip>
        }
      >
        <p className="text-[15px] font-semibold text-ink">{t("sources.googleSheets")}</p>
        <p className="mt-1 font-mono text-[12.5px] leading-5 text-ink-muted">
          {t("sources.linkedTo", { email: account.email })}
        </p>
      </SourceCardShell>
      <ConfirmDialog
        open={isConfirmOpen}
        onOpenChange={setIsConfirmOpen}
        title={t("sources.disconnectTitle")}
        description={t("sources.disconnectDescription")}
        confirmLabel={t("sources.disconnect")}
        isPending={disconnect.isPending}
        onConfirm={() =>
          disconnect.mutate(account.sourceId, { onSettled: () => setIsConfirmOpen(false) })
        }
      />
    </>
  );
}

/** Dashed affordance to link another Google account. Disabled until the OAuth
 * client id AND secret are configured; the tooltip points to Settings. */
function AddGoogleAccountCard({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: config } = useGoogleConfig();
  const connect = useGoogleConnect();
  const { t } = useTranslation();

  const hasClientId = (config?.clientId ?? null) !== null;
  const hasClientSecret = config?.hasClientSecret ?? false;
  const isConfigured = hasClientId && hasClientSecret;
  const isBusy = connect.isPending;

  const button = (
    <button
      type="button"
      disabled={!isConfigured || isBusy}
      onClick={() => connect.mutate()}
      className={cn(
        "flex min-h-40 w-full flex-col items-center justify-center gap-2 rounded-card border border-dashed border-edge-strong",
        "px-5 py-6 text-center text-ink-muted transition-colors",
        FOCUS_RING,
        isConfigured && !isBusy
          ? "hover:border-accent hover:text-accent"
          : "cursor-not-allowed opacity-70"
      )}
    >
      {isBusy ? (
        <Loader2 size={20} aria-hidden className="animate-spin" />
      ) : (
        <Plus size={20} aria-hidden />
      )}
      <span className="text-[13px] font-medium">
        {isBusy ? t("sources.connecting") : t("sources.addGoogleAccount")}
      </span>
      <span className="max-w-56 text-[12px] leading-4">
        {isBusy
          ? t("sources.finishSignIn")
          : t("sources.addGoogleAccountHint")}
      </span>
    </button>
  );

  if (isConfigured) {
    return button;
  }

  return (
    <div className="flex flex-col gap-2">
      <Tooltip>
        <TooltipTrigger asChild>
          {/* Span wrapper so the tooltip still fires over a disabled button. */}
          <span className="inline-flex">{button}</span>
        </TooltipTrigger>
        <TooltipContent>
          {hasClientId
            ? t("sources.saveSecretFirst")
            : t("sources.setClientIdFirst")}
        </TooltipContent>
      </Tooltip>
      <button
        type="button"
        onClick={() => onNavigate("settings")}
        className={cn(
          "self-center rounded text-[12px] font-medium text-accent transition-colors hover:text-accent-hover",
          FOCUS_RING
        )}
      >
        {t("sources.configureGoogle")}
      </button>
    </div>
  );
}

function ProviderCard() {
  const { t } = useTranslation();
  return (
    <SourceCardShell
      overline="provider"
      badge={<Badge variant="muted">{t("sources.comingSoon")}</Badge>}
      footer={
        <>
          <Tooltip>
            <TooltipTrigger asChild>
              {/* Span wrapper so the tooltip still fires over a disabled button. */}
              <span className="inline-flex">
                <Button variant="outline" size="sm" disabled>
                  {t("sources.connect")}
                </Button>
              </span>
            </TooltipTrigger>
            <TooltipContent>{t("sources.connectTooltip")}</TooltipContent>
          </Tooltip>
          <span className="text-[12px] text-ink-muted">{t("sources.notAvailableYet")}</span>
        </>
      }
    >
      <p className="text-[15px] font-semibold text-ink">{t("sources.additionalProvider")}</p>
      <p className="mt-1 text-[13px] leading-5 text-ink-muted">
        {t("sources.additionalProviderHint")}
      </p>
    </SourceCardShell>
  );
}

/** Any already-persisted source without dedicated connect UI (e.g. mock). */
function GenericSourceCard({ source }: { source: DataSource }) {
  const { t } = useTranslation();
  const status = source.status ?? "placeholder";
  const statusLabel = t(STATUS_LABEL_KEYS[status]);
  return (
    <SourceCardShell
      overline={source.kind}
      badge={<Badge variant={STATUS_VARIANTS[status]}>{statusLabel}</Badge>}
      footer={
        <Button variant="secondary" size="sm" disabled>
          {statusLabel}
        </Button>
      }
    >
      <p className="text-[15px] font-semibold text-ink">{source.name}</p>
      <p className="mt-1 text-[13px] leading-5 text-ink-muted">
        {status === "connected"
          ? t("sources.genericConnected")
          : t("sources.genericPlaceholder")}
      </p>
    </SourceCardShell>
  );
}

export function DataSources({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { t } = useTranslation();
  const { data: sources, isPending: isSourcesPending } = useSources();
  const { data: accounts, isPending: isAccountsPending } = useGoogleAccounts();
  const isPending = isSourcesPending || isAccountsPending;

  const list = sources ?? [];
  const googleAccounts = accounts ?? [];
  // Non-Google sources still render their own cards below the Google cluster.
  const otherSources = list.filter((source) => source.kind !== "google_sheets");

  return (
    <>
      <ScreenHeader
        title={t("screen.sources.title")}
        description={t("screen.sources.description")}
      />
      {isPending ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <Skeleton className="h-40 rounded-card" />
          <Skeleton className="h-40 rounded-card" />
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {googleAccounts.map((account) => (
            <GoogleAccountCard key={account.sourceId} account={account} />
          ))}
          <AddGoogleAccountCard onNavigate={onNavigate} />
          <ProviderCard />
          {otherSources.map((source) => (
            <GenericSourceCard key={source.id} source={source} />
          ))}
        </div>
      )}
    </>
  );
}
