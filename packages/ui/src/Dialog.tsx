import * as DialogPrimitive from "@radix-ui/react-dialog";
import {
  forwardRef,
  type ComponentPropsWithoutRef,
  type ElementRef,
  type HTMLAttributes
} from "react";
import { cn } from "./cn.js";
import { XIcon } from "./icons.js";

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export const DialogContent = forwardRef<
  ElementRef<typeof DialogPrimitive.Content>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Content>
>(({ className, children, ...props }, ref) => (
  <DialogPrimitive.Portal>
    <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-bg/70 backdrop-blur-sm motion-safe:animate-fade-in" />
    <DialogPrimitive.Content
      ref={ref}
      className={cn(
        "fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2",
        "rounded-lg border border-edge bg-surface p-5 shadow-raised",
        "focus:outline-none motion-safe:animate-scale-in",
        className
      )}
      {...props}
    >
      {children}
      <DialogPrimitive.Close
        aria-label="Close dialog"
        className={cn(
          "absolute right-3.5 top-3.5 rounded-md p-1 text-ink-muted transition-colors",
          "hover:bg-raised hover:text-ink",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60"
        )}
      >
        <XIcon className="h-3.5 w-3.5" />
      </DialogPrimitive.Close>
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
));
DialogContent.displayName = "DialogContent";

export function DialogHeader({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("mb-4 flex flex-col gap-1 pr-8", className)} {...props} />;
}

export const DialogTitle = forwardRef<
  ElementRef<typeof DialogPrimitive.Title>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn("text-base font-semibold text-ink", className)}
    {...props}
  />
));
DialogTitle.displayName = "DialogTitle";

export const DialogDescription = forwardRef<
  ElementRef<typeof DialogPrimitive.Description>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description
    ref={ref}
    className={cn("text-[13px] text-ink-muted", className)}
    {...props}
  />
));
DialogDescription.displayName = "DialogDescription";

export function DialogFooter({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("mt-5 flex items-center justify-end gap-2", className)} {...props} />;
}
