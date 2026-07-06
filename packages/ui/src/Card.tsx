import type { HTMLAttributes } from "react";
import { cn } from "./cn.js";

/** Raised card: 10px radius, hairline edge, soft light-mode shadow. */
export function Card({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return (
    <section
      className={cn("rounded-card border border-edge bg-raised shadow-card", className)}
      {...props}
    />
  );
}

/** Header strip: overline label on the left, optional action on the right. */
export function CardHeader({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("flex items-center justify-between gap-3 border-b border-edge px-5 py-3", className)}
      {...props}
    />
  );
}

export function CardTitle({ className, ...props }: HTMLAttributes<HTMLHeadingElement>) {
  return <h3 className={cn("overline-label", className)} {...props} />;
}

export function CardContent({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("px-5 py-4", className)} {...props} />;
}
