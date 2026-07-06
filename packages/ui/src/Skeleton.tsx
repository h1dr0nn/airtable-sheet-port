import type { HTMLAttributes } from "react";
import { cn } from "./cn.js";

export function Skeleton({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      aria-hidden
      className={cn("rounded-md bg-raised motion-safe:animate-pulse", className)}
      {...props}
    />
  );
}
