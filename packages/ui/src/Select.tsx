import * as SelectPrimitive from "@radix-ui/react-select";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "./cn.js";
import { CheckIcon, ChevronDownIcon, ChevronUpIcon } from "./icons.js";

export const Select = SelectPrimitive.Root;
export const SelectValue = SelectPrimitive.Value;

export const SelectTrigger = forwardRef<
  ElementRef<typeof SelectPrimitive.Trigger>,
  ComponentPropsWithoutRef<typeof SelectPrimitive.Trigger>
>(({ className, children, ...props }, ref) => (
  <SelectPrimitive.Trigger
    ref={ref}
    className={cn(
      "flex h-9 items-center justify-between gap-2 rounded-md border border-edge-strong bg-bg px-3",
      "text-sm text-ink transition-colors hover:bg-raised",
      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60",
      "disabled:cursor-not-allowed disabled:opacity-45 data-[placeholder]:text-ink-muted",
      className
    )}
    {...props}
  >
    {children}
    <SelectPrimitive.Icon>
      <ChevronDownIcon className="h-3.5 w-3.5 text-ink-muted" />
    </SelectPrimitive.Icon>
  </SelectPrimitive.Trigger>
));
SelectTrigger.displayName = "SelectTrigger";

export const SelectContent = forwardRef<
  ElementRef<typeof SelectPrimitive.Content>,
  ComponentPropsWithoutRef<typeof SelectPrimitive.Content>
>(({ className, children, position = "popper", ...props }, ref) => (
  <SelectPrimitive.Portal>
    <SelectPrimitive.Content
      ref={ref}
      position={position}
      className={cn(
        "z-50 min-w-[var(--radix-select-trigger-width)] overflow-hidden rounded-md border border-edge-strong",
        "bg-raised shadow-raised motion-safe:animate-fade-in",
        className
      )}
      {...props}
    >
      <SelectPrimitive.ScrollUpButton className="flex h-6 items-center justify-center text-ink-muted">
        <ChevronUpIcon className="h-3.5 w-3.5" />
      </SelectPrimitive.ScrollUpButton>
      <SelectPrimitive.Viewport className="p-1">{children}</SelectPrimitive.Viewport>
      <SelectPrimitive.ScrollDownButton className="flex h-6 items-center justify-center text-ink-muted">
        <ChevronDownIcon className="h-3.5 w-3.5" />
      </SelectPrimitive.ScrollDownButton>
    </SelectPrimitive.Content>
  </SelectPrimitive.Portal>
));
SelectContent.displayName = "SelectContent";

export const SelectItem = forwardRef<
  ElementRef<typeof SelectPrimitive.Item>,
  ComponentPropsWithoutRef<typeof SelectPrimitive.Item>
>(({ className, children, ...props }, ref) => (
  <SelectPrimitive.Item
    ref={ref}
    className={cn(
      "flex cursor-pointer select-none items-center justify-between gap-2 rounded px-2 py-1.5",
      "text-[13px] text-ink outline-none transition-colors",
      "data-[highlighted]:bg-surface data-[state=checked]:text-accent",
      "data-[disabled]:pointer-events-none data-[disabled]:opacity-45",
      className
    )}
    {...props}
  >
    <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
    <SelectPrimitive.ItemIndicator>
      <CheckIcon className="h-3 w-3" />
    </SelectPrimitive.ItemIndicator>
  </SelectPrimitive.Item>
));
SelectItem.displayName = "SelectItem";
