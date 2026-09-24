import { useState, type ReactNode } from "react";
import { Plus } from "lucide-react";
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
import { useGoogleAccounts, useRemoveGoogleBridge } from "../hooks/useGoogleBridges.js";
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

/** One connected Google account: its email plus a confirmed Disconnect, which
 * removes the account's bridge. */
function GoogleAccountCard({ account }: { account: GoogleAccount }) {
  const disconnect = useRemoveGoogleBridge();
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

/** Dashed affordance to link another Google account. Accounts are added as
 * Apps Script bridges, so it opens the Google Bridges card in Settings. */
function AddGoogleAccountCard({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      onClick={() => onNavigate("settings")}
      className={cn(
        "flex min-h-40 w-full flex-col items-center justify-center gap-2 rounded-card border border-dashed border-edge-strong",
        "px-5 py-6 text-center text-ink-muted transition-colors hover:border-accent hover:text-accent",
        FOCUS_RING
      )}
    >
      <Plus size={20} aria-hidden />
      <span className="text-[13px] font-medium">{t("sources.addGoogleAccount")}</span>
      <span className="max-w-56 text-[12px] leading-4">{t("sources.addGoogleAccountHint")}</span>
    </button>
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
          {otherSources.map((source) => (
            <GenericSourceCard key={source.id} source={source} />
          ))}
        </div>
      )}
    </>
  );
}
