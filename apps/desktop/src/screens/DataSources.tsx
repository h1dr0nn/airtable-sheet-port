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
import type { GoogleAccount } from "../lib/ipc.js";
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

/** One connected Google account: its email plus a confirmed Disconnect. */
function GoogleAccountCard({ account }: { account: GoogleAccount }) {
  const disconnect = useGoogleDisconnect();
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  return (
    <>
      <SourceCardShell
        overline="google_sheets"
        badge={<Badge variant="success">Connected</Badge>}
        footer={
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                disabled={disconnect.isPending}
                onClick={() => setIsConfirmOpen(true)}
              >
                Disconnect
              </Button>
            </TooltipTrigger>
            <TooltipContent>Remove this account and its stored token</TooltipContent>
          </Tooltip>
        }
      >
        <p className="text-[15px] font-semibold text-ink">Google Sheets</p>
        <p className="mt-1 text-[13px] leading-5 text-ink-muted">
          Linked to <span className="font-mono text-[12.5px] text-ink">{account.email}</span>
        </p>
      </SourceCardShell>
      <ConfirmDialog
        open={isConfirmOpen}
        onOpenChange={setIsConfirmOpen}
        title="Disconnect Google Account?"
        description="Agents lose access to this account's spreadsheets and the stored token is removed from the OS keychain. You can reconnect at any time."
        confirmLabel="Disconnect"
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
        {isBusy ? "Connecting..." : "Add Google Account"}
      </span>
      <span className="max-w-56 text-[12px] leading-4">
        {isBusy
          ? "Finish signing in with Google in your browser"
          : "Link another Google account so agents can reach more spreadsheets"}
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
            ? "Save the OAuth client secret in Settings first"
            : "Set the OAuth client ID in Settings first"}
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
        Configure Google in Settings
      </button>
    </div>
  );
}

function ProviderCard() {
  return (
    <SourceCardShell
      overline="provider"
      badge={<Badge variant="muted">Coming Soon</Badge>}
      footer={
        <>
          <Tooltip>
            <TooltipTrigger asChild>
              {/* Span wrapper so the tooltip still fires over a disabled button. */}
              <span className="inline-flex">
                <Button variant="outline" size="sm" disabled>
                  Connect
                </Button>
              </span>
            </TooltipTrigger>
            <TooltipContent>Available once the connector ships</TooltipContent>
          </Tooltip>
          <span className="text-[12px] text-ink-muted">Not available yet</span>
        </>
      }
    >
      <p className="text-[15px] font-semibold text-ink">Additional Provider</p>
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
