import type { HTMLAttributes } from "react";
import { cn } from "./cn.js";

/** Static square placeholder. No shimmer: the substrate does not animate. */
export function Skeleton({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div aria-hidden className={cn("bg-raised", className)} {...props} />;
}
