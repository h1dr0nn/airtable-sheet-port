import { cn } from "./cn.js";

export type StatusDotStatus = "live" | "idle" | "alert";

// "live" is terminal green, reserved for the MCP RUNNING readout.
// Only the live dot may pulse (motion-safe).
const DOT_CLASSES: Record<StatusDotStatus, string> = {
  live: "bg-signal motion-safe:animate-dot-pulse",
  idle: "bg-ink-muted",
  alert: "bg-hazard"
};

type StatusDotProps = {
  status: StatusDotStatus;
  className?: string;
};

/** Square 6x6px status indicator. */
export function StatusDot({ status, className }: StatusDotProps) {
  return <span className={cn("inline-block h-1.5 w-1.5 shrink-0", DOT_CLASSES[status], className)} aria-hidden />;
}
