import type { ReactNode } from "react";
import { Badge, Button, type BadgeVariant } from "@sheet-port/ui";
import type { DataSource, SourceStatus } from "@sheet-port/shared";
import { useSources } from "../hooks/useSources.js";
import { useTranslation } from "../i18n/useTranslation.js";
import type { TranslationKey } from "../i18n/translations.js";
import { GoogleBridgesCard } from "../components/sources/GoogleBridgesCard.js";
import { ScreenHeader } from "../components/ScreenHeader.js";
import type { ScreenId } from "../lib/nav.js";

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

/** Data sources: the Google bridge pool (add, test, remove) plus any other
 * persisted source. Google accounts are managed here and nowhere else. */
export function DataSources({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { t } = useTranslation();
  const { data: sources } = useSources();
  // Google accounts are listed by the bridges card; other kinds get a card each.
  const otherSources = (sources ?? []).filter((source) => source.kind !== "google_sheets");

  return (
    <>
      <ScreenHeader
        title={t("screen.sources.title")}
        description={t("screen.sources.description")}
      />
      <div className="space-y-4">
        <GoogleBridgesCard onOpenGuide={() => onNavigate("guide")} />
        {otherSources.length > 0 ? (
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            {otherSources.map((source) => (
              <GenericSourceCard key={source.id} source={source} />
            ))}
          </div>
        ) : null}
      </div>
    </>
  );
}
