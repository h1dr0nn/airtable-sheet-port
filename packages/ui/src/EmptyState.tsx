import type { ReactNode } from "react";
import { cn } from "./cn.js";

type EmptyStateProps = {
  title: string;
  description?: string;
  icon?: ReactNode;
  action?: ReactNode;
  className?: string;
};

export function EmptyState({ title, description, icon, action, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-edge-strong",
        "px-6 py-12 text-center",
        className
      )}
    >
      {icon ? <div className="mb-1 text-ink-muted/70">{icon}</div> : null}
      <p className="text-sm font-medium text-ink">{title}</p>
      {description ? <p className="max-w-sm text-[13px] text-ink-muted">{description}</p> : null}
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}
