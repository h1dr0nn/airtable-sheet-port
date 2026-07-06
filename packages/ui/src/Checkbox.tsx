import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "./cn.js";
import { CheckIcon } from "./icons.js";

export const Checkbox = forwardRef<
  ElementRef<typeof CheckboxPrimitive.Root>,
  ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root>
>(({ className, ...props }, ref) => (
  <CheckboxPrimitive.Root
    ref={ref}
    className={cn(
      "flex h-4 w-4 shrink-0 items-center justify-center rounded border border-edge-strong bg-bg",
      "transition-colors data-[state=checked]:border-accent data-[state=checked]:bg-accent",
      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60",
      "focus-visible:ring-offset-2 focus-visible:ring-offset-bg",
      "disabled:cursor-not-allowed disabled:opacity-45",
      className
    )}
    {...props}
  >
    <CheckboxPrimitive.Indicator className="text-bg">
      <CheckIcon className="h-3 w-3" />
    </CheckboxPrimitive.Indicator>
  </CheckboxPrimitive.Root>
));
Checkbox.displayName = "Checkbox";
