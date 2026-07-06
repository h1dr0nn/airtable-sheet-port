import type { HTMLAttributes } from "react";
import { cn } from "./cn.js";

/** Placeholder block with a subtle motion-safe shimmer. */
export function Skeleton({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      aria-hidden
      className={cn("rounded-md bg-edge/70 motion-safe:animate-pulse", className)}
      {...props}
    />
  );
}
