import type { HTMLAttributes } from "react";
import { cn } from "./cn.js";

export function Card({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return (
    <section
      className={cn("rounded-lg border border-edge bg-surface shadow-card", className)}
      {...props}
    />
  );
}

export function CardHeader({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("flex items-center justify-between gap-3 px-4 pt-4", className)} {...props} />;
}

/** Section label style: small caps, wide tracking, muted. */
export function CardTitle({ className, ...props }: HTMLAttributes<HTMLHeadingElement>) {
  return (
    <h3
      className={cn("text-[11px] font-semibold uppercase tracking-widest text-ink-muted", className)}
      {...props}
    />
  );
}

export function CardContent({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("px-4 pb-4 pt-3", className)} {...props} />;
}
