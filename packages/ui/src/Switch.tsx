import * as SwitchPrimitive from "@radix-ui/react-switch";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "./cn.js";

/** Bordered rectangular track with a square sliding thumb. */
export const Switch = forwardRef<
  ElementRef<typeof SwitchPrimitive.Root>,
  ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitive.Root
    ref={ref}
    className={cn(
      "inline-flex h-4 w-8 shrink-0 cursor-pointer items-center border border-edge-strong",
      "bg-bg transition-colors data-[state=checked]:border-ink",
      "focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2",
      "focus-visible:outline-hazard",
      "disabled:cursor-not-allowed disabled:opacity-40",
      className
    )}
    {...props}
  >
    <SwitchPrimitive.Thumb
      className={cn(
        "block h-2.5 w-2.5 translate-x-[3px] bg-ink-muted transition-transform",
        "data-[state=checked]:translate-x-[17px] data-[state=checked]:bg-ink"
      )}
    />
  </SwitchPrimitive.Root>
));
Switch.displayName = "Switch";
