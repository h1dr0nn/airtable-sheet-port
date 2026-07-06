import * as SwitchPrimitive from "@radix-ui/react-switch";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "./cn.js";
import { FOCUS_RING } from "./focus.js";

export const Switch = forwardRef<
  ElementRef<typeof SwitchPrimitive.Root>,
  ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitive.Root
    ref={ref}
    className={cn(
      "inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full",
      "bg-edge-strong transition-colors data-[state=checked]:bg-accent",
      FOCUS_RING,
      "disabled:cursor-not-allowed disabled:opacity-50",
      className
    )}
    {...props}
  >
    <SwitchPrimitive.Thumb
      className={cn(
        "block h-4 w-4 translate-x-0.5 rounded-full bg-white shadow-sm transition-transform",
        "data-[state=checked]:translate-x-[18px]"
      )}
    />
  </SwitchPrimitive.Root>
));
Switch.displayName = "Switch";
