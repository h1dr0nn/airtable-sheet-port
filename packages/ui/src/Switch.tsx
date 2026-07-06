import * as SwitchPrimitive from "@radix-ui/react-switch";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "./cn.js";

export const Switch = forwardRef<
  ElementRef<typeof SwitchPrimitive.Root>,
  ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitive.Root
    ref={ref}
    className={cn(
      "inline-flex h-[18px] w-8 shrink-0 cursor-pointer items-center rounded-full border border-edge-strong",
      "bg-raised transition-colors data-[state=checked]:border-accent/40 data-[state=checked]:bg-accent",
      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60",
      "focus-visible:ring-offset-2 focus-visible:ring-offset-bg",
      "disabled:cursor-not-allowed disabled:opacity-45",
      className
    )}
    {...props}
  >
    <SwitchPrimitive.Thumb
      className={cn(
        "block h-3 w-3 translate-x-0.5 rounded-full bg-ink-muted shadow-sm transition-transform",
        "data-[state=checked]:translate-x-[15px] data-[state=checked]:bg-bg"
      )}
    />
  </SwitchPrimitive.Root>
));
Switch.displayName = "Switch";
