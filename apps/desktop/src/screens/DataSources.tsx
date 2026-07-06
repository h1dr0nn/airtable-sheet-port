import { useState, type ReactNode } from "react";
import { Loader2 } from "lucide-react";
import { Badge, Button, cn, FOCUS_RING, Skeleton, type BadgeVariant } from "@sheet-port/ui";
import type { DataSource, SourceStatus } from "@sheet-port/shared";
import { useGoogleConfig, useGoogleConnect, useGoogleDisconnect } from "../hooks/useGoogleConfig.js";
import { useSources } from "../hooks/useSources.js";
import type { ScreenId } from "../lib/nav.js";
import { ConfirmDialog } from "../components/ConfirmDialog.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const STATUS_VARIANTS: Record<SourceStatus, BadgeVariant> = {
  connected: "success",
  placeholder: "muted",
  error: "danger"
};

const STATUS_LABELS: Record<SourceStatus, string> = {
  connected: "Connected",
  placeholder: "Placeholder",
  error: "Error"
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

type GoogleSheetsCardProps = {
  source: DataSource | undefined;
  onNavigate: (screen: ScreenId) => void;
};

function GoogleSheetsCard({ source, onNavigate }: GoogleSheetsCardProps) {
  const { data: config } = useGoogleConfig();
  const connect = useGoogleConnect();
  const disconnect = useGoogleDisconnect();
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  const isConnected = source !== undefined && source.status === "connected";
  const hasClientId = (config?.clientId ?? null) !== null;
  const email = config?.connectedEmail;

  if (isConnected) {
    return (
      <>
        <SourceCardShell
          overline="google_sheets"
          badge={<Badge variant="success">Connected</Badge>}
          footer={
            <Button
              variant="outline"
              size="sm"
              disabled={disconnect.isPending}
              onClick={() => setIsConfirmOpen(true)}
            >
              Disconnect
            </Button>
          }
        >
          <p className="text-[15px] font-semibold text-ink">{source.name}</p>
          <p className="mt-1 text-[13px] leading-5 text-ink-muted">
            {email ? (
              <>
                Linked to <span className="font-mono text-[12.5px] text-ink">{email}</span>
              </>
            ) : (
              "Available to agents through permission rules"
            )}
          </p>
        </SourceCardShell>
        <ConfirmDialog
          open={isConfirmOpen}
          onOpenChange={setIsConfirmOpen}
          title="Disconnect Google Sheets?"
          description="Agents lose access to this account's spreadsheets and the stored token is removed from the OS keychain. You can reconnect at any time."
          confirmLabel="Disconnect"
          isPending={disconnect.isPending}
          onConfirm={() =>
            disconnect.mutate(undefined, { onSettled: () => setIsConfirmOpen(false) })
          }
        />
      </>
    );
  }

  return (
    <SourceCardShell
      overline="google_sheets"
      badge={<Badge variant="muted">Not connected</Badge>}
      footer={
        <>
          <Button
            variant="outline"
            size="sm"
            disabled={!hasClientId || connect.isPending}
            onClick={() => connect.mutate()}
          >
            {connect.isPending ? (
              <>
                <Loader2 size={14} aria-hidden className="animate-spin" />
                Connecting...
              </>
            ) : (
              "Connect"
            )}
          </Button>
          {!hasClientId ? (
            <button
              type="button"
              onClick={() => onNavigate("settings")}
              className={cn(
                "rounded text-[12px] font-medium text-accent transition-colors hover:text-accent-hover",
                FOCUS_RING
              )}
            >
              Set client ID in Settings
            </button>
          ) : null}
        </>
      }
    >
      <p className="text-[15px] font-semibold text-ink">Google Sheets</p>
      <p className="mt-1 text-[13px] leading-5 text-ink-muted">
        {connect.isPending
          ? "Finish signing in with Google in your browser; this card updates when consent completes"
          : "Link a Google account so agents can read and preview writes to your spreadsheets"}
      </p>
    </SourceCardShell>
  );
}

function ProviderCard() {
  return (
    <SourceCardShell
      overline="provider"
      badge={<Badge variant="muted">Coming soon</Badge>}
      footer={
        <>
          <Button variant="outline" size="sm" disabled>
            Connect
          </Button>
          <span className="text-[12px] text-ink-muted">Not available yet</span>
        </>
      }
    >
      <p className="text-[15px] font-semibold text-ink">Additional provider</p>
      <p className="mt-1 text-[13px] leading-5 text-ink-muted">
        A second table provider lands here once its connector ships
      </p>
    </SourceCardShell>
  );
}

/** Any already-persisted source without dedicated connect UI (e.g. mock). */
function GenericSourceCard({ source }: { source: DataSource }) {
  const status = source.status ?? "placeholder";
  return (
    <SourceCardShell
      overline={source.kind}
      badge={<Badge variant={STATUS_VARIANTS[status]}>{STATUS_LABELS[status]}</Badge>}
      footer={
        <Button variant="secondary" size="sm" disabled>
          {STATUS_LABELS[status]}
        </Button>
      }
    >
      <p className="text-[15px] font-semibold text-ink">{source.name}</p>
      <p className="mt-1 text-[13px] leading-5 text-ink-muted">
        {status === "connected"
          ? "Available to agents through permission rules"
          : "Connector scaffolded; authentication is not wired up yet"}
      </p>
    </SourceCardShell>
  );
}

export function DataSources({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: sources, isPending } = useSources();
  const list = sources ?? [];
  const googleSource = list.find((source) => source.kind === "google_sheets");
  const otherSources = list.filter((source) => source.kind !== "google_sheets");

  return (
    <>
      <ScreenHeader
        title="Data Sources"
        description="Connect table providers here; agents only ever see what permission rules allow"
      />
      {isPending ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <Skeleton className="h-40 rounded-card" />
          <Skeleton className="h-40 rounded-card" />
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <GoogleSheetsCard source={googleSource} onNavigate={onNavigate} />
          <ProviderCard />
          {otherSources.map((source) => (
            <GenericSourceCard key={source.id} source={source} />
          ))}
        </div>
      )}
    </>
  );
}
