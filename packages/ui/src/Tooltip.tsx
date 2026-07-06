import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "./cn.js";

export const TooltipProvider = TooltipPrimitive.Provider;
export const Tooltip = TooltipPrimitive.Root;
export const TooltipTrigger = TooltipPrimitive.Trigger;

const TOOLTIP_SIDE_OFFSET = 6;

export const TooltipContent = forwardRef<
  ElementRef<typeof TooltipPrimitive.Content>,
  ComponentPropsWithoutRef<typeof TooltipPrimitive.Content>
>(({ className, sideOffset = TOOLTIP_SIDE_OFFSET, children, ...props }, ref) => (
  <TooltipPrimitive.Portal>
    <TooltipPrimitive.Content
      ref={ref}
      sideOffset={sideOffset}
      // Rides the dropdown layer so it escapes dialogs, the titlebar, and the
      // toast stack. Kept in sync with --z-dropdown in styles.css.
      style={{ zIndex: "var(--z-dropdown)" }}
      className={cn(
        "max-w-xs rounded-md border border-edge bg-raised px-2 py-1 shadow-pop",
        "font-sans text-[11.5px] leading-4 text-ink motion-safe:animate-fade-in",
        className
      )}
      {...props}
    >
      {children}
      <TooltipPrimitive.Arrow className="fill-raised" width={9} height={4} />
    </TooltipPrimitive.Content>
  </TooltipPrimitive.Portal>
));
TooltipContent.displayName = "TooltipContent";

type TooltipHintProps = {
  /** Keyboard shortcut rendered in mono, e.g. "Ctrl K". */
  children: string;
};

/** Mono keyboard-hint pill for use inside TooltipContent. */
export function TooltipHint({ children }: TooltipHintProps) {
  return (
    <span className="ml-1.5 rounded border border-edge bg-surface px-1 py-px font-mono text-[10.5px] text-ink-muted">
      {children}
    </span>
  );
}
