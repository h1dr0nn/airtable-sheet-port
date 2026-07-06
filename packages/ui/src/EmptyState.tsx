import type { ReactNode } from "react";
import { cn } from "./cn.js";

type EmptyStateProps = {
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
};

/** Centered "[ NO RECORDS ]" style readout inside a hairline compartment. */
export function EmptyState({ title, description, action, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-2 border border-edge bg-surface",
        "px-6 py-14 text-center",
        className
      )}
    >
      <p className="font-mono text-[13px] font-bold uppercase tracking-[0.1em] text-ink">
        {"[ "}
        {title}
        {" ]"}
      </p>
      {description ? (
        <p className="max-w-sm font-mono text-[11px] uppercase tracking-[0.05em] text-ink-muted">
          {description}
        </p>
      ) : null}
      {action ? <div className="mt-3">{action}</div> : null}
    </div>
  );
}
