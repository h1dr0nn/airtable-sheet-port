import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "./cn.js";

export type InputProps = InputHTMLAttributes<HTMLInputElement>;

export const Input = forwardRef<HTMLInputElement, InputProps>(({ className, ...props }, ref) => (
  <input
    ref={ref}
    className={cn(
      "h-9 w-full rounded-md border border-edge-strong bg-bg px-3 text-sm text-ink",
      "placeholder:text-ink-muted/70 transition-colors",
      "hover:border-edge-strong focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60",
      "disabled:cursor-not-allowed disabled:opacity-45",
      className
    )}
    {...props}
  />
));
Input.displayName = "Input";
