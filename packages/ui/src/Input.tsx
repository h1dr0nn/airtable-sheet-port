import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "./cn.js";

export type InputProps = InputHTMLAttributes<HTMLInputElement>;

export const Input = forwardRef<HTMLInputElement, InputProps>(({ className, ...props }, ref) => (
  <input
    ref={ref}
    className={cn(
      "h-9 w-full border border-edge-strong bg-bg px-3 font-mono text-[13px] text-ink",
      "placeholder:text-ink-muted transition-colors hover:border-ink",
      "focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2",
      "focus-visible:outline-hazard",
      "disabled:cursor-not-allowed disabled:opacity-40",
      className
    )}
    {...props}
  />
));
Input.displayName = "Input";
