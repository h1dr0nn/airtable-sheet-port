import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "./cn.js";
import { FOCUS_RING } from "./focus.js";

export type InputProps = InputHTMLAttributes<HTMLInputElement>;

export const Input = forwardRef<HTMLInputElement, InputProps>(({ className, ...props }, ref) => (
  <input
    ref={ref}
    className={cn(
      "h-9 w-full rounded-md border border-edge-strong bg-bg px-3 font-sans text-[13px] text-ink",
      "placeholder:text-ink-faint transition-colors hover:border-ink-faint",
      FOCUS_RING,
      "disabled:cursor-not-allowed disabled:opacity-50",
      className
    )}
    {...props}
  />
));
Input.displayName = "Input";
