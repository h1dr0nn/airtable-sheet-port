import type { ReactNode } from "react";
import { cn } from "./cn.js";

type EmptyStateProps = {
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
};

/** Quiet centered message for empty collections. */
export function EmptyState({ title, description, action, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center rounded-card border border-edge bg-surface",
        "px-6 py-14 text-center",
        className
      )}
    >
      <p className="text-[13px] font-medium text-ink">{title}</p>
      {description ? <p className="mt-1 max-w-sm text-[13px] text-ink-muted">{description}</p> : null}
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}
