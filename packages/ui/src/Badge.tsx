import { cva, type VariantProps } from "class-variance-authority";
import type { HTMLAttributes } from "react";
import { cn } from "./cn.js";

const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium leading-4",
  {
    variants: {
      variant: {
        default: "border-edge-strong bg-raised text-ink",
        success: "border-accent/25 bg-accent/10 text-accent",
        warning: "border-warning/25 bg-warning/10 text-warning",
        danger: "border-danger/25 bg-danger/10 text-danger",
        info: "border-info/25 bg-info/10 text-info",
        muted: "border-edge bg-surface text-ink-muted"
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
