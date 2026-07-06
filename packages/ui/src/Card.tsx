import type { HTMLAttributes } from "react";
import { cn } from "./cn.js";

/** Bordered compartment: 1px hairline frame, square, panel fill. */
export function Card({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return <section className={cn("border border-edge bg-surface", className)} {...props} />;
}

/** Header strip separated from the body by a hairline. */
export function CardHeader({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("flex items-center justify-between gap-3 border-b border-edge px-4 py-2", className)}
      {...props}
    />
  );
}

/** Compartment label in "[ LABEL ]" ASCII framing. */
export function CardTitle({ className, children, ...props }: HTMLAttributes<HTMLHeadingElement>) {
  return (
    <h3
      className={cn("font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-ink-muted", className)}
      {...props}
    >
      {"[ "}
      {children}
      {" ]"}
    </h3>
  );
}

export function CardContent({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("px-4 py-3", className)} {...props} />;
}
