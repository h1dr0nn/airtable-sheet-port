import { useState } from "react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Skeleton,
  StatusDot,
  type StatusDotStatus,
  Tooltip,
  TooltipContent,
  TooltipTrigger
} from "@sheet-port/ui";
import {
  useConfigureAllMcpClients,
  useConfigureMcpClient,
  useMcpClients,
  useUnregisterMcpClient
} from "../../hooks/useMcp.js";
import type { BadgeVariant } from "@sheet-port/ui";
import { useTranslation } from "../../i18n/useTranslation.js";
import type { TranslationKey } from "../../i18n/translations.js";
import type { McpClient, McpClientState } from "../../lib/ipc.js";
import { ConfirmDialog } from "../ConfirmDialog.js";

type StatePresentation = {
  labelKey: TranslationKey;
  dot: StatusDotStatus;
  badge: BadgeVariant;
};

// Maps the backend client state onto its dot color and badge, per the task spec:
// Configured green, Missing config amber, Not Found muted.
const STATE_PRESENTATION: Record<McpClientState, StatePresentation> = {
  configured: { labelKey: "settings.mcpClients.stateConfigured", dot: "live", badge: "success" },
  unconfigured: { labelKey: "settings.mcpClients.stateMissingConfig", dot: "alert", badge: "warning" },
  not_found: { labelKey: "settings.mcpClients.stateNotFound", dot: "idle", badge: "muted" }
};

// Fallback so an unexpected state can never white-screen the whole app.
const UNKNOWN_PRESENTATION: StatePresentation = {
  labelKey: "settings.mcpClients.stateUnknown",
  dot: "idle",
  badge: "muted"
};

type ClientDetailProps = {
  client: McpClient;
};

/** Status row and action pair for the selected client. */
function ClientDetail({ client }: ClientDetailProps) {
  const configure = useConfigureMcpClient();
  const unregister = useUnregisterMcpClient();
  const { t } = useTranslation();
  const [isUnregisterConfirmOpen, setIsUnregisterConfirmOpen] = useState(false);

  const presentation = STATE_PRESENTATION[client.state] ?? UNKNOWN_PRESENTATION;
  const isInstalled = client.state !== "not_found";
  const isConfigured = client.state === "configured";
  const isBusy = configure.isPending || unregister.isPending;

  const configureButton = (
    <Button
      size="sm"
      disabled={!isInstalled || isConfigured || isBusy}
      onClick={() => configure.mutate(client.id)}
    >
      {configure.isPending ? t("settings.mcpClients.configuring") : t("settings.mcpClients.configure")}
    </Button>
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
        <div className="flex items-center gap-2">
          <StatusDot status={presentation.dot} />
          <span className="text-[13px] font-medium text-ink">{client.name}</span>
          <Badge variant={presentation.badge}>{t(presentation.labelKey)}</Badge>
        </div>
        <div className="flex items-center gap-2">
          {!isInstalled ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="inline-flex">{configureButton}</span>
              </TooltipTrigger>
              <TooltipContent>{t("settings.mcpClients.notInstalled", { name: client.name })}</TooltipContent>
            </Tooltip>
          ) : isConfigured ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="inline-flex">{configureButton}</span>
              </TooltipTrigger>
              <TooltipContent>{t("settings.mcpClients.alreadyConfigured")}</TooltipContent>
            </Tooltip>
          ) : (
            configureButton
          )}
          <Button
            variant="outline"
            size="sm"
            disabled={!isConfigured || isBusy}
            onClick={() => setIsUnregisterConfirmOpen(true)}
          >
            {t("settings.mcpClients.unregister")}
          </Button>
        </div>
      </div>

      {client.configPath ? (
        <div className="flex items-center gap-2 border-t border-edge pt-3">
          <span className="shrink-0 text-[12px] text-ink-muted">{t("settings.mcpClients.configFile")}</span>
          <Tooltip>
            <TooltipTrigger asChild>
              <code className="min-w-0 truncate font-mono text-[12px] text-ink-muted">
                {client.configPath}
              </code>
            </TooltipTrigger>
            <TooltipContent className="max-w-md break-all font-mono">
              {client.configPath}
            </TooltipContent>
          </Tooltip>
        </div>
      ) : null}

      <ConfirmDialog
        open={isUnregisterConfirmOpen}
        onOpenChange={setIsUnregisterConfirmOpen}
        title={t("settings.mcpClients.unregisterTitle", { name: client.name })}
        description={t("settings.mcpClients.unregisterDescription", { name: client.name })}
        confirmLabel={t("settings.mcpClients.unregister")}
        isPending={unregister.isPending}
        onConfirm={() =>
          unregister.mutate(client.id, { onSettled: () => setIsUnregisterConfirmOpen(false) })
        }
      />
    </div>
  );
}

/** Detect + auto-configure known MCP clients (Claude Desktop, Cursor, ...).
 * Modeled on the MCP-for-Unity client cluster, restyled to the app tokens. */
export function McpClientsCard() {
  const { data: clients, isPending } = useMcpClients();
  const configureAll = useConfigureAllMcpClients();
  const { t } = useTranslation();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const clientList = clients ?? [];
  // Default the dropdown to the first detected client once data arrives.
  const selected =
    clientList.find((client) => client.id === selectedId) ?? clientList[0] ?? null;

  const hasUnconfigured = clientList.some((client) => client.state === "unconfigured");
  const configureAllButton = (
    <Button
      variant="secondary"
      size="sm"
      disabled={!hasUnconfigured || configureAll.isPending}
      onClick={() => configureAll.mutate()}
    >
      {configureAll.isPending ? t("settings.mcpClients.configuring") : t("settings.mcpClients.configureAll")}
    </Button>
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.mcpClients.title")}</CardTitle>
      </CardHeader>
      <CardContent>
        {isPending || !clients ? (
          <Skeleton className="h-40" />
        ) : clientList.length === 0 ? (
          <p className="rounded-md border border-edge bg-surface px-3 py-6 text-center text-[12.5px] text-ink-muted">
            {t("settings.mcpClients.noneDetected")}
          </p>
        ) : (
          <div className="space-y-4">
            <div className="space-y-1.5">
              <label className="text-[12px] font-medium text-ink-muted" htmlFor="mcp-client">
                {t("settings.mcpClients.client")}
              </label>
              <Select
                value={selected?.id}
                onValueChange={(value) => setSelectedId(value)}
              >
                <SelectTrigger id="mcp-client" className="w-full" aria-label={t("settings.mcpClients.clientAria")}>
                  <SelectValue placeholder={t("settings.mcpClients.selectClient")} />
                </SelectTrigger>
                <SelectContent>
                  {clientList.map((client) => (
                    <SelectItem key={client.id} value={client.id}>
                      {client.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {selected ? <ClientDetail client={selected} /> : null}

            <div className="flex justify-end border-t border-edge pt-4">
              {hasUnconfigured ? (
                configureAllButton
              ) : (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span className="inline-flex">{configureAllButton}</span>
                  </TooltipTrigger>
                  <TooltipContent>{t("settings.mcpClients.noneNeedConfigure")}</TooltipContent>
                </Tooltip>
              )}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
