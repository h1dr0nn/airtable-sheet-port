import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "./cn.js";

const buttonVariants = cva(
  [
    "inline-flex select-none items-center justify-center gap-2 whitespace-nowrap",
    "font-mono text-[11px] font-bold uppercase tracking-[0.08em] transition-colors",
    "focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2",
    "focus-visible:outline-hazard",
    "disabled:pointer-events-none disabled:opacity-40"
  ].join(" "),
  {
    variants: {
      variant: {
        // Primary (approve): solid phosphor, hover pure white.
        default: "border border-ink bg-ink text-bg hover:border-white hover:bg-white",
        // Standard: phosphor border/text, hover inverts.
        secondary: "border border-ink bg-transparent text-ink hover:bg-ink hover:text-bg",
        ghost: "border border-transparent text-ink-muted hover:border-edge-strong hover:text-ink",
        // Destructive: hazard red, hover fills red.
        destructive: "border border-hazard bg-transparent text-hazard hover:bg-hazard hover:text-bg",
        outline: "border border-edge-strong bg-transparent text-ink hover:border-ink hover:bg-ink hover:text-bg"
      },
      size: {
        sm: "h-7 px-3 text-[10px]",
        md: "h-9 px-4",
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
