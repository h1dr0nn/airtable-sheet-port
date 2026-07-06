import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
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
import type { McpClient, McpClientState } from "../../lib/ipc.js";
import { MCP_STDIO_COMMAND } from "../../lib/constants.js";
import { ConfirmDialog } from "../ConfirmDialog.js";

type StatePresentation = {
  label: string;
  dot: StatusDotStatus;
  badge: BadgeVariant;
};

// Maps the backend client state onto its dot color and badge, per the task spec:
// Configured green, Missing config amber, Not Found muted.
const STATE_PRESENTATION: Record<McpClientState, StatePresentation> = {
  configured: { label: "Configured", dot: "live", badge: "success" },
  unconfigured: { label: "Missing Config", dot: "alert", badge: "warning" },
  not_found: { label: "Not Found", dot: "idle", badge: "muted" }
};

// Fallback so an unexpected state can never white-screen the whole app.
const UNKNOWN_PRESENTATION: StatePresentation = {
  label: "Unknown",
  dot: "idle",
  badge: "muted"
};

type ClientDetailProps = {
  client: McpClient;
};

/** Status row, action pair, and editable binary path for the selected client. */
function ClientDetail({ client }: ClientDetailProps) {
  const configure = useConfigureMcpClient();
  const unregister = useUnregisterMcpClient();
  const [isUnregisterConfirmOpen, setIsUnregisterConfirmOpen] = useState(false);

  // Binary path defaults to the resolved sidecar path; editable per client so the
  // user can point a client at a custom build. null draft = "not edited yet".
  const [pathDraft, setPathDraft] = useState<string | null>(null);
  // Reset the local edit whenever the selected client changes.
  useEffect(() => {
    setPathDraft(null);
  }, [client.id]);

  const presentation = STATE_PRESENTATION[client.state] ?? UNKNOWN_PRESENTATION;
  const isInstalled = client.state !== "not_found";
  const isConfigured = client.state === "configured";
  const isBusy = configure.isPending || unregister.isPending;

  const binaryPath = pathDraft ?? MCP_STDIO_COMMAND;
  const isPathDirty = pathDraft !== null && pathDraft !== MCP_STDIO_COMMAND;

  const configureButton = (
    <Button
      size="sm"
      disabled={!isInstalled || isConfigured || isBusy}
      onClick={() => configure.mutate(client.id)}
    >
      {configure.isPending ? "Configuring..." : "Configure"}
    </Button>
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
        <div className="flex items-center gap-2">
          <StatusDot status={presentation.dot} />
          <span className="text-[13px] font-medium text-ink">{client.name}</span>
          <Badge variant={presentation.badge}>{presentation.label}</Badge>
        </div>
        <div className="flex items-center gap-2">
          {!isInstalled ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="inline-flex">{configureButton}</span>
              </TooltipTrigger>
              <TooltipContent>{client.name} is not installed</TooltipContent>
            </Tooltip>
          ) : isConfigured ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="inline-flex">{configureButton}</span>
              </TooltipTrigger>
              <TooltipContent>Already configured</TooltipContent>
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
            Unregister
          </Button>
        </div>
      </div>

      <div className="space-y-1.5">
        <label className="text-[12px] font-medium text-ink-muted" htmlFor="mcp-binary-path">
          Server Binary Path
        </label>
        <div className="flex items-center gap-2">
          <Input
            id="mcp-binary-path"
            className="font-mono text-[12.5px]"
            value={binaryPath}
            spellCheck={false}
            autoComplete="off"
            disabled={!isInstalled}
            onChange={(event) => setPathDraft(event.target.value)}
          />
          {isPathDirty ? (
            <Button variant="outline" size="sm" onClick={() => setPathDraft(null)}>
              Reset
            </Button>
          ) : null}
        </div>
        <p className="text-[12px] leading-4 text-ink-muted">
          Path written into {client.name}'s config. Defaults to the resolved sidecar binary.
        </p>
      </div>

      {client.configPath ? (
        <div className="flex items-center gap-2 border-t border-edge pt-3">
          <span className="shrink-0 text-[12px] text-ink-muted">Config file</span>
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
        title={`Unregister From ${client.name}?`}
        description={`This edits ${client.name}'s config file to remove the Sheet Port MCP server. You can reconfigure it at any time.`}
        confirmLabel="Unregister"
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
      {configureAll.isPending ? "Configuring..." : "Configure All Detected Clients"}
    </Button>
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle>MCP Clients</CardTitle>
      </CardHeader>
      <CardContent>
        {isPending || !clients ? (
          <Skeleton className="h-40" />
        ) : clientList.length === 0 ? (
          <p className="rounded-md border border-edge bg-surface px-3 py-6 text-center text-[12.5px] text-ink-muted">
            No supported MCP clients detected
          </p>
        ) : (
          <div className="space-y-4">
            <div className="space-y-1.5">
              <label className="text-[12px] font-medium text-ink-muted" htmlFor="mcp-client">
                Client
              </label>
              <Select
                value={selected?.id}
                onValueChange={(value) => setSelectedId(value)}
              >
                <SelectTrigger id="mcp-client" className="w-full" aria-label="MCP Client">
                  <SelectValue placeholder="Select a client" />
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
                  <TooltipContent>No detected clients need configuring</TooltipContent>
                </Tooltip>
              )}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
