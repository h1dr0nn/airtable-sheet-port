import * as DialogPrimitive from "@radix-ui/react-dialog";
import {
  forwardRef,
  type ComponentPropsWithoutRef,
  type ElementRef,
  type HTMLAttributes
} from "react";
import { cn } from "./cn.js";
import { FOCUS_RING } from "./focus.js";
import { XIcon } from "./icons.js";

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export const DialogContent = forwardRef<
  ElementRef<typeof DialogPrimitive.Content>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Content>
>(({ className, children, ...props }, ref) => (
  <DialogPrimitive.Portal>
    <DialogPrimitive.Overlay
      // Sits below the 40px titlebar (see --z-titlebar) so window controls stay
      // clickable while a dialog is open.
      style={{ zIndex: "var(--z-modal-overlay)" }}
      className="fixed inset-0 bg-overlay/50 motion-safe:animate-fade-in"
    />
    <DialogPrimitive.Content
      ref={ref}
      // Anchored below the titlebar rather than viewport-centered so it never
      // overlaps the custom bar; horizontally centered as before.
      style={{ zIndex: "var(--z-modal)", top: "calc(40px + 10vh)" }}
      className={cn(
        "fixed left-1/2 w-full max-w-md -translate-x-1/2",
        "rounded-card border border-edge bg-raised p-5 shadow-pop",
        "focus:outline-none motion-safe:animate-fade-in",
        className
      )}
      {...props}
    >
      {children}
      <DialogPrimitive.Close
        aria-label="Close dialog"
        className={cn(
          "absolute right-3 top-3 rounded-md p-1 text-ink-muted transition-colors",
          "hover:bg-surface hover:text-ink",
          FOCUS_RING
        )}
      >
        <XIcon className="h-4 w-4" />
      </DialogPrimitive.Close>
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
));
DialogContent.displayName = "DialogContent";

export function DialogHeader({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("mb-4 flex flex-col gap-1.5 pr-8", className)} {...props} />;
}

export const DialogTitle = forwardRef<
  ElementRef<typeof DialogPrimitive.Title>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn("text-[16px] font-semibold leading-6 text-ink", className)}
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
    className={cn("text-[13px] leading-5 text-ink-muted", className)}
    {...props}
  />
));
DialogDescription.displayName = "DialogDescription";

export function DialogFooter({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("mt-5 flex items-center justify-end gap-2", className)} {...props} />;
}
