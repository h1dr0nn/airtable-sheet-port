import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "./cn.js";
import { FOCUS_RING } from "./focus.js";

const buttonVariants = cva(
  [
    "inline-flex select-none items-center justify-center gap-2 whitespace-nowrap rounded-md",
    "font-sans font-medium transition-colors",
    FOCUS_RING,
    "disabled:pointer-events-none disabled:opacity-50"
  ].join(" "),
  {
    variants: {
      variant: {
        // Primary: solid copper.
        default: "bg-accent text-accent-ink hover:bg-accent-hover",
        secondary: "border border-edge-strong bg-surface text-ink hover:bg-raised",
        ghost: "text-ink-muted hover:bg-surface hover:text-ink",
        destructive: "bg-danger-solid text-white hover:bg-danger-solid-hover",
        outline: "border border-edge-strong bg-transparent text-ink hover:bg-surface"
      },
      size: {
        sm: "h-8 px-3 text-[12.5px]",
        md: "h-9 px-4 text-[13px]",
        icon: "h-8 w-8"
      }
    },
    defaultVariants: {
      variant: "default",
      size: "md"
    }
  }
);

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & VariantProps<typeof buttonVariants>;

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, type = "button", ...props }, ref) => (
    <button ref={ref} type={type} className={cn(buttonVariants({ variant, size }), className)} {...props} />
  )
);
Button.displayName = "Button";
