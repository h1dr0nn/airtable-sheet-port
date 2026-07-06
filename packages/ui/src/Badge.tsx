import { cva, type VariantProps } from "class-variance-authority";
import type { HTMLAttributes } from "react";
import { cn } from "./cn.js";

// Bordered uppercase mono labels. No tinted backgrounds, no rounding.
const badgeVariants = cva(
  "inline-flex items-center gap-1 border px-1.5 py-0.5 font-mono text-[10px] font-medium uppercase leading-none tracking-[0.08em]",
  {
    variants: {
      variant: {
        default: "border-edge-strong text-ink",
        strong: "border-ink text-ink",
        danger: "border-hazard text-hazard",
        muted: "border-edge text-ink-muted"
      }
    },
    defaultVariants: {
      variant: "default"
    }
  }
);

export type BadgeVariant = NonNullable<VariantProps<typeof badgeVariants>["variant"]>;

export type BadgeProps = HTMLAttributes<HTMLSpanElement> & VariantProps<typeof badgeVariants>;

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}
