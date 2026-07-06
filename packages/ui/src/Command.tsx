import * as DialogPrimitive from "@radix-ui/react-dialog";
import { Command as CommandPrimitive } from "cmdk";
import {
  forwardRef,
  type ComponentPropsWithoutRef,
  type ElementRef,
  type ReactNode
} from "react";
import { cn } from "./cn.js";
import { SearchIcon } from "./icons.js";

// shadcn "Command" pattern: cmdk primitives styled with the app tokens, plus
// a dialog wrapper that floats the palette near the top of the viewport.

export const Command = forwardRef<
  ElementRef<typeof CommandPrimitive>,
  ComponentPropsWithoutRef<typeof CommandPrimitive>
>(({ className, ...props }, ref) => (
  <CommandPrimitive
    ref={ref}
    className={cn("flex w-full flex-col overflow-hidden bg-raised text-ink", className)}
    {...props}
  />
));
Command.displayName = "Command";

type CommandDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Accessible dialog name; visually hidden. */
  title: string;
  children: ReactNode;
};

/** Centered command-palette shell: dimmed overlay, panel at 25vh, max-w-xl. */
export function CommandDialog({ open, onOpenChange, title, children }: CommandDialogProps) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-overlay/50 motion-safe:animate-fade-in" />
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className={cn(
            "fixed left-1/2 top-[25vh] z-50 w-full max-w-xl -translate-x-1/2 overflow-hidden",
            "rounded-card border border-edge bg-raised p-0 shadow-pop",
            "focus:outline-none motion-safe:animate-fade-in"
          )}
        >
          <DialogPrimitive.Title className="sr-only">{title}</DialogPrimitive.Title>
          {children}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}

export const CommandInput = forwardRef<
  ElementRef<typeof CommandPrimitive.Input>,
  ComponentPropsWithoutRef<typeof CommandPrimitive.Input>
>(({ className, ...props }, ref) => (
  <div className="flex items-center gap-2.5 border-b border-edge px-3.5" cmdk-input-wrapper="">
    <SearchIcon className="h-4 w-4 shrink-0 text-ink-faint" />
    <CommandPrimitive.Input
      ref={ref}
      className={cn(
        "h-11 w-full bg-transparent font-sans text-[13px] text-ink outline-none",
        "placeholder:text-ink-faint disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    />
  </div>
));
CommandInput.displayName = "CommandInput";

export const CommandList = forwardRef<
  ElementRef<typeof CommandPrimitive.List>,
  ComponentPropsWithoutRef<typeof CommandPrimitive.List>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.List
    ref={ref}
    className={cn("max-h-80 overflow-y-auto overflow-x-hidden p-1.5", className)}
    {...props}
  />
));
CommandList.displayName = "CommandList";

export const CommandEmpty = forwardRef<
  ElementRef<typeof CommandPrimitive.Empty>,
  ComponentPropsWithoutRef<typeof CommandPrimitive.Empty>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.Empty
    ref={ref}
    className={cn("py-6 text-center text-[12.5px] text-ink-muted", className)}
    {...props}
  />
));
CommandEmpty.displayName = "CommandEmpty";

export const CommandGroup = forwardRef<
  ElementRef<typeof CommandPrimitive.Group>,
  ComponentPropsWithoutRef<typeof CommandPrimitive.Group>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.Group
    ref={ref}
    className={cn(
      "overflow-hidden [&_[cmdk-group-heading]]:px-2.5 [&_[cmdk-group-heading]]:pb-1",
      "[&_[cmdk-group-heading]]:pt-2 [&_[cmdk-group-heading]]:text-[11px]",
      "[&_[cmdk-group-heading]]:font-semibold [&_[cmdk-group-heading]]:uppercase",
      "[&_[cmdk-group-heading]]:tracking-wide [&_[cmdk-group-heading]]:text-ink-faint",
      className
    )}
    {...props}
  />
));
CommandGroup.displayName = "CommandGroup";

export const CommandSeparator = forwardRef<
  ElementRef<typeof CommandPrimitive.Separator>,
  ComponentPropsWithoutRef<typeof CommandPrimitive.Separator>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.Separator
    ref={ref}
    className={cn("mx-1 my-1 h-px bg-edge", className)}
    {...props}
  />
));
CommandSeparator.displayName = "CommandSeparator";

export const CommandItem = forwardRef<
  ElementRef<typeof CommandPrimitive.Item>,
  ComponentPropsWithoutRef<typeof CommandPrimitive.Item>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.Item
    ref={ref}
    className={cn(
      "flex cursor-default select-none items-center gap-2.5 rounded-md px-2.5 py-1.5",
      "text-[13px] text-ink outline-none transition-colors",
      "data-[selected=true]:bg-accent/10 data-[selected=true]:text-ink",
      "data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-50",
      className
    )}
    {...props}
  />
));
CommandItem.displayName = "CommandItem";
