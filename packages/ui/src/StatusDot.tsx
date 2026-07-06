import { cn } from "./cn.js";

export type StatusDotStatus = "live" | "idle" | "warning" | "danger";

const DOT_CLASSES: Record<StatusDotStatus, string> = {
  live: "bg-accent",
  idle: "bg-ink-muted/60",
  warning: "bg-warning",
  danger: "bg-danger"
};

type StatusDotProps = {
  status: StatusDotStatus;
  className?: string;
};

/** Small status indicator; "live" pulses (respects reduced motion). */
export function StatusDot({ status, className }: StatusDotProps) {
  return (
    <span className={cn("relative inline-flex h-2 w-2", className)} aria-hidden>
      {status === "live" ? (
        <span className="absolute inline-flex h-full w-full rounded-full bg-accent opacity-60 motion-safe:animate-ping" />
      ) : null}
      <span className={cn("relative inline-flex h-2 w-2 rounded-full", DOT_CLASSES[status])} />
    </span>
  );
}
