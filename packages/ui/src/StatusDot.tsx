import { cn } from "./cn.js";

export type StatusDotStatus = "live" | "idle" | "alert";

// Only the live dot may pulse (motion-safe).
const DOT_CLASSES: Record<StatusDotStatus, string> = {
  live: "bg-success motion-safe:animate-dot-pulse",
  idle: "bg-ink-faint",
  alert: "bg-danger"
};

type StatusDotProps = {
  status: StatusDotStatus;
  className?: string;
};

/** Round 8x8px status indicator. */
export function StatusDot({ status, className }: StatusDotProps) {
  return (
    <span
      className={cn("inline-block h-2 w-2 shrink-0 rounded-full", DOT_CLASSES[status], className)}
      aria-hidden
    />
  );
}
