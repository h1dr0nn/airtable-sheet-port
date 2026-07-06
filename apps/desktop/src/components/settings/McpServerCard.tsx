import { useState } from "react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Skeleton,
  StatusDot,
  Tooltip,
  TooltipContent,
  TooltipTrigger
} from "@sheet-port/ui";
import { Loader2 } from "lucide-react";
import {
  useMcpConfig,
  useSetMcpPort,
  useSetMcpTransport,
  useStartMcpServer,
  useStopMcpServer
} from "../../hooks/useMcp.js";
import type { McpTransport } from "../../lib/ipc.js";
import { MCP_PORT_MAX, MCP_PORT_MIN, buildMcpHttpUrl } from "../../lib/constants.js";
import { CopyButton } from "../CopyButton.js";
import { SegmentedControl, type SegmentedOption } from "../SegmentedControl.js";

const TRANSPORT_OPTIONS: ReadonlyArray<SegmentedOption<McpTransport>> = [
  { value: "stdio", label: "Stdio" },
  { value: "http", label: "Local HTTP" }
];

/** Validates the port draft against the backend bounds. Returns an error string
 * when invalid so the field can explain why Save stays disabled. */
function validatePort(draft: string): { port: number } | { error: string } {
  const trimmed = draft.trim();
  if (trimmed === "") {
    return { error: "Enter a port" };
  }
  const port = Number(trimmed);
  if (!Number.isInteger(port)) {
    return { error: "Port must be a whole number" };
  }
  if (port < MCP_PORT_MIN || port > MCP_PORT_MAX) {
    return { error: `Port must be between ${MCP_PORT_MIN} and ${MCP_PORT_MAX}` };
  }
  return { port };
}

type PortFieldProps = {
  savedPort: number;
};

/** Port input shown only for Local HTTP; dirty-checked against the saved port. */
function PortField({ savedPort }: PortFieldProps) {
  const setPort = useSetMcpPort();
  // null draft = "not edited yet"; the input then mirrors the saved port.
  const [draft, setDraft] = useState<string | null>(null);

  const value = draft ?? String(savedPort);
  const result = validatePort(value);
  const isValid = "port" in result;
  const isDirty = isValid && result.port !== savedPort;
  const canSave = isDirty && !setPort.isPending;

  const disabledReason = !isValid
    ? result.error
    : !isDirty
      ? "No changes to save"
      : "";

  const save = () => {
    if (!("port" in result)) {
      return;
    }
    setPort.mutate(result.port, { onSuccess: () => setDraft(null) });
  };

  const saveButton = (
    <Button size="sm" disabled={!canSave} onClick={save}>
      {setPort.isPending ? "Saving..." : "Save"}
    </Button>
  );

  return (
    <div className="space-y-1.5">
      <label className="text-[12px] font-medium text-ink-muted" htmlFor="mcp-port">
        HTTP Port
      </label>
      <div className="flex items-center gap-2">
        <Input
          id="mcp-port"
          className="max-w-32 font-mono text-[12.5px]"
          inputMode="numeric"
          value={value}
          placeholder="4319"
          spellCheck={false}
          autoComplete="off"
          aria-invalid={!isValid}
          onChange={(event) => setDraft(event.target.value)}
        />
        {canSave || setPort.isPending ? (
          saveButton
        ) : (
          <Tooltip>
            <TooltipTrigger asChild>
              {/* Span wrapper so the tooltip fires over the disabled button. */}
              <span className="inline-flex">{saveButton}</span>
            </TooltipTrigger>
            <TooltipContent>{disabledReason}</TooltipContent>
          </Tooltip>
        )}
      </div>
      <p className="text-[12px] leading-4 text-ink-muted">
        Loopback port for the local HTTP endpoint. Range {MCP_PORT_MIN}-{MCP_PORT_MAX}.
      </p>
    </div>
  );
}

/** Start/Stop control for the desktop-managed HTTP sidecar. Stdio clients
 * spawn their own sidecar, so this only applies to the Local HTTP transport. */
function ServerControl({ isRunning }: { isRunning: boolean }) {
  const start = useStartMcpServer();
  const stop = useStopMcpServer();
  const isBusy = start.isPending || stop.isPending;

  return (
    <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 border-t border-edge pt-4">
      <div className="min-w-0">
        <p className="text-[13px] font-medium text-ink">Server Process</p>
        <p className="mt-0.5 text-[12.5px] leading-4 text-ink-muted">
          Runs the shared HTTP endpoint as a desktop-managed process.
        </p>
      </div>
      {isRunning ? (
        <Button
          size="sm"
          variant="outline"
          disabled={isBusy}
          onClick={() => stop.mutate()}
        >
          {stop.isPending ? (
            <>
              <Loader2 size={14} aria-hidden className="animate-spin" />
              Stopping...
            </>
          ) : (
            "Stop"
          )}
        </Button>
      ) : (
        <Button size="sm" disabled={isBusy} onClick={() => start.mutate()}>
          {start.isPending ? (
            <>
              <Loader2 size={14} aria-hidden className="animate-spin" />
              Starting...
            </>
          ) : (
            "Start"
          )}
        </Button>
      )}
    </div>
  );
}

/** MCP sidecar status plus transport/port controls. Changing either only takes
 * effect after the sidecar restarts, so the card says so explicitly. */
export function McpServerCard() {
  const { data: config, isPending } = useMcpConfig();
  const setTransport = useSetMcpTransport();

  const isRunning = config?.running ?? false;
  const transport = config?.transport ?? "stdio";
  const port = config?.port ?? MCP_PORT_MIN;
  const isHttp = transport === "http";

  return (
    <Card>
      <CardHeader>
        <CardTitle>MCP Server</CardTitle>
        {isPending || !config ? null : (
          <div className="flex items-center gap-1.5">
            <StatusDot status={isRunning ? "live" : "idle"} />
            <Badge variant={isRunning ? "success" : "muted"}>
              {isRunning ? "Running" : "Offline"}
            </Badge>
          </div>
        )}
      </CardHeader>
      <CardContent>
        {isPending || !config ? (
          <Skeleton className="h-40" />
        ) : (
          <div className="space-y-4">
            <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
              <div className="min-w-0">
                <p className="text-[13px] font-medium text-ink">Transport</p>
                <p className="mt-0.5 text-[12.5px] leading-4 text-ink-muted">
                  Stdio spawns the sidecar per client; Local HTTP serves one shared endpoint.
                </p>
              </div>
              <SegmentedControl
                options={TRANSPORT_OPTIONS}
                value={transport}
                onChange={(next) => setTransport.mutate(next)}
                ariaLabel="MCP Transport"
              />
            </div>

            {isHttp ? <PortField savedPort={port} /> : null}

            {isHttp ? (
              <ServerControl isRunning={isRunning} />
            ) : (
              <p className="border-t border-edge pt-4 text-[12px] leading-4 text-ink-muted">
                Clients launch the sidecar themselves - use MCP Clients to register it.
              </p>
            )}

            {isHttp ? (
              <div className="space-y-1.5 border-t border-edge pt-4">
                <p className="text-[12px] font-medium text-ink-muted">Endpoint URL</p>
                <div className="flex items-center gap-2">
                  <code className="flex-1 truncate rounded-md border border-edge bg-surface px-2.5 py-1.5 font-mono text-[12.5px] text-ink">
                    {buildMcpHttpUrl(port)}
                  </code>
                  <CopyButton value={buildMcpHttpUrl(port)} label="Copy endpoint URL" />
                </div>
                <p className="text-[12px] leading-4 text-ink-muted">
                  Changing the transport or port requires restarting the sidecar to take effect.
                </p>
              </div>
            ) : null}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
