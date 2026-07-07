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
import { useTranslation } from "../../i18n/useTranslation.js";
import type { TranslationKey } from "../../i18n/translations.js";
import { MCP_PORT_MAX, MCP_PORT_MIN, buildMcpHttpUrl } from "../../lib/constants.js";
import { CopyButton } from "../CopyButton.js";
import { SegmentedControl, type SegmentedOption } from "../SegmentedControl.js";

type PortValidation =
  | { port: number }
  | { errorKey: TranslationKey; params?: Record<string, number> };

/** Validates the port draft against the backend bounds. Returns an error key
 * when invalid so the field can explain why Save stays disabled. */
function validatePort(draft: string): PortValidation {
  const trimmed = draft.trim();
  if (trimmed === "") {
    return { errorKey: "settings.mcpServer.enterPort" };
  }
  const port = Number(trimmed);
  if (!Number.isInteger(port)) {
    return { errorKey: "settings.mcpServer.portWholeNumber" };
  }
  if (port < MCP_PORT_MIN || port > MCP_PORT_MAX) {
    return {
      errorKey: "settings.mcpServer.portRange",
      params: { min: MCP_PORT_MIN, max: MCP_PORT_MAX }
    };
  }
  return { port };
}

type PortFieldProps = {
  savedPort: number;
};

/** Port input shown only for Local HTTP; dirty-checked against the saved port. */
function PortField({ savedPort }: PortFieldProps) {
  const setPort = useSetMcpPort();
  const { t } = useTranslation();
  // null draft = "not edited yet"; the input then mirrors the saved port.
  const [draft, setDraft] = useState<string | null>(null);

  const value = draft ?? String(savedPort);
  const result = validatePort(value);
  const isValid = "port" in result;
  const isDirty = isValid && result.port !== savedPort;
  const canSave = isDirty && !setPort.isPending;

  const disabledReason = !isValid
    ? t(result.errorKey, result.params)
    : !isDirty
      ? t("common.noChangesToSave")
      : "";

  const save = () => {
    if (!("port" in result)) {
      return;
    }
    setPort.mutate(result.port, { onSuccess: () => setDraft(null) });
  };

  const saveButton = (
    <Button size="sm" disabled={!canSave} onClick={save}>
      {setPort.isPending ? t("common.saving") : t("common.save")}
    </Button>
  );

  return (
    <div className="space-y-1.5">
      <label className="text-[12px] font-medium text-ink-muted" htmlFor="mcp-port">
        {t("settings.mcpServer.httpPort")}
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
        {t("settings.mcpServer.httpPortHint", { min: MCP_PORT_MIN, max: MCP_PORT_MAX })}
      </p>
    </div>
  );
}

/** Start/Stop control for the desktop-managed sidecar. The backend now respects
 * the configured transport, so this applies to both stdio and Local HTTP: the
 * description adapts to explain what the managed process does for each. */
function ServerControl({ isRunning, isHttp }: { isRunning: boolean; isHttp: boolean }) {
  const start = useStartMcpServer();
  const stop = useStopMcpServer();
  const { t } = useTranslation();
  const isBusy = start.isPending || stop.isPending;

  return (
    <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 border-t border-edge pt-4">
      <div className="min-w-0">
        <p className="text-[13px] font-medium text-ink">{t("settings.mcpServer.serverProcess")}</p>
        <p className="mt-0.5 text-[12.5px] leading-4 text-ink-muted">
          {isHttp
            ? t("settings.mcpServer.serverProcessHttp")
            : t("settings.mcpServer.serverProcessStdio")}
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
              {t("settings.mcpServer.stopping")}
            </>
          ) : (
            t("settings.mcpServer.stop")
          )}
        </Button>
      ) : (
        <Button size="sm" disabled={isBusy} onClick={() => start.mutate()}>
          {start.isPending ? (
            <>
              <Loader2 size={14} aria-hidden className="animate-spin" />
              {t("settings.mcpServer.starting")}
            </>
          ) : (
            t("settings.mcpServer.start")
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
  const { t } = useTranslation();
  const transportOptions: ReadonlyArray<SegmentedOption<McpTransport>> = [
    { value: "stdio", label: t("settings.mcpServer.transportStdio") },
    { value: "http", label: t("settings.mcpServer.transportHttp") }
  ];

  const isRunning = config?.running ?? false;
  const transport = config?.transport ?? "stdio";
  const port = config?.port ?? MCP_PORT_MIN;
  const isHttp = transport === "http";

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.mcpServer.title")}</CardTitle>
        {isPending || !config ? null : (
          <div className="flex items-center gap-1.5">
            <StatusDot status={isRunning ? "live" : "idle"} />
            <Badge variant={isRunning ? "success" : "muted"}>
              {isRunning ? t("common.running") : t("common.offline")}
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
                <p className="text-[13px] font-medium text-ink">{t("settings.mcpServer.transport")}</p>
                <p className="mt-0.5 text-[12.5px] leading-4 text-ink-muted">
                  {t("settings.mcpServer.transportDescription")}
                </p>
              </div>
              <SegmentedControl
                options={transportOptions}
                value={transport}
                onChange={(next) => setTransport.mutate(next)}
                ariaLabel={t("settings.mcpServer.transportAria")}
              />
            </div>

            {isHttp ? <PortField savedPort={port} /> : null}

            <ServerControl isRunning={isRunning} isHttp={isHttp} />

            {isHttp ? (
              <div className="space-y-1.5 border-t border-edge pt-4">
                <p className="text-[12px] font-medium text-ink-muted">{t("settings.mcpServer.endpointUrl")}</p>
                <div className="flex items-center gap-2">
                  <code className="flex-1 truncate rounded-md border border-edge bg-surface px-2.5 py-1.5 font-mono text-[12.5px] text-ink">
                    {buildMcpHttpUrl(port)}
                  </code>
                  <CopyButton value={buildMcpHttpUrl(port)} label={t("settings.mcpServer.copyEndpoint")} />
                </div>
                <p className="text-[12px] leading-4 text-ink-muted">
                  {t("settings.mcpServer.restartHint")}
                </p>
              </div>
            ) : null}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
