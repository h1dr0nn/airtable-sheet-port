import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "./cn.js";

const buttonVariants = cva(
  [
    "inline-flex select-none items-center justify-center gap-2 whitespace-nowrap rounded-md",
    "text-sm font-medium transition-colors",
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60",
    "focus-visible:ring-offset-2 focus-visible:ring-offset-bg",
    "disabled:pointer-events-none disabled:opacity-45"
  ].join(" "),
  {
    variants: {
      variant: {
        default: "bg-accent text-bg hover:bg-accent/85 active:bg-accent/75",
        secondary: "border border-edge-strong bg-raised text-ink hover:bg-raised/70 active:bg-raised/60",
        ghost: "text-ink-muted hover:bg-raised hover:text-ink active:bg-raised/70",
        destructive: "bg-danger text-bg hover:bg-danger/85 active:bg-danger/75",
        outline: "border border-edge-strong bg-transparent text-ink hover:bg-raised active:bg-raised/70"
      },
      size: {
        sm: "h-7 px-2.5 text-xs",
        md: "h-9 px-3.5",
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
