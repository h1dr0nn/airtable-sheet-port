import { useCallback, useEffect, useRef, useState, type ComponentType } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import {
  AlertTriangleIcon,
  CheckIcon,
  ChevronRightIcon,
  CopyIcon,
  InfoIcon,
  LoaderIcon,
  XIcon
} from "../icons.js";
import {
  nextToastId,
  useToastStore,
  type ToastAction,
  type ToastItem,
  type ToastVariant
} from "./toastStore.js";

type IconComponent = ComponentType<{ className?: string }>;

interface VariantStyle {
  icon: IconComponent;
  color: string;
  spin?: boolean;
}

const VARIANTS: Record<ToastVariant, VariantStyle> = {
  success: { icon: CheckIcon, color: "text-success" },
  error: { icon: AlertTriangleIcon, color: "text-danger" },
  info: { icon: InfoIcon, color: "text-accent" },
  default: { icon: InfoIcon, color: "text-ink-faint" },
  loading: { icon: LoaderIcon, color: "text-ink-faint", spin: true }
};

const DEFAULT_DURATION = 4000;
const COPIED_RESET_MS = 1000;
/** How far a card slides in from the right; zeroed under reduced motion. */
const SLIDE_IN_X = 40;

interface AppToastOptions {
  /** The notification source/area, e.g. "Export failed", "Saved". */
  title: string;
  /** Optional detail line under the title. */
  description?: string;
  variant?: ToastVariant;
  /** Render the description as a click-to-copy code block (errors, paths).
   * Defaults to true only for the error variant. Ignored when `action` is set. */
  copyable?: boolean;
  /** Add a direct-action text link to a plain description row. */
  action?: ToastAction;
  duration?: number;
  /** Stable id, so a loading toast can be updated or dismissed by reference. */
  id?: string | number;
}

/** Show an app notification. Single entry point so every toast stays consistent. */
export function appToast({
  title,
  description,
  variant = "default",
  copyable,
  action,
  duration,
  id
}: AppToastOptions): string {
  const toastId = id != null ? String(id) : nextToastId();
  useToastStore.getState().add({
    id: toastId,
    title,
    description,
    variant,
    copyable: action ? false : copyable ?? variant === "error",
    action,
    duration: duration ?? (variant === "loading" ? Infinity : DEFAULT_DURATION)
  });
  return toastId;
}

appToast.dismiss = (id?: string | number) =>
  useToastStore.getState().remove(id == null ? undefined : String(id));

function CopyableDescription({ description }: { description: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard
      ?.writeText(description)
      .then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), COPIED_RESET_MS);
      })
      .catch(() => {
        // Clipboard can be unavailable; copy is best-effort.
      });
  };

  return (
    <div
      role="button"
      tabIndex={0}
      title="Click to copy"
      onClick={handleCopy}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") handleCopy();
      }}
      className="group flex cursor-pointer items-start gap-1.5 rounded-md border border-transparent
        bg-surface px-2 py-1.5 transition-colors hover:border-edge hover:bg-bg"
    >
      <span className="line-clamp-2 min-w-0 flex-1 break-all font-mono text-[11.5px] leading-[1.5] text-ink-muted group-hover:line-clamp-none">
        {description}
      </span>
      {copied ? (
        <CheckIcon className="mt-px h-[13px] w-[13px] shrink-0 text-success" />
      ) : (
        <CopyIcon className="mt-px h-[13px] w-[13px] shrink-0 text-ink-faint transition-colors group-hover:text-accent" />
      )}
    </div>
  );
}

function PlainDescription({
  description,
  action,
  onDismiss
}: {
  description: string;
  action?: ToastAction;
  onDismiss: () => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <span className="min-w-0 flex-1 break-words text-[12px] leading-[1.45] text-ink-muted">
        {description}
      </span>
      {action && (
        <button
          type="button"
          onClick={() => {
            action.onClick();
            onDismiss();
          }}
          className="flex shrink-0 items-center gap-0.5 whitespace-nowrap text-[12px] font-medium text-accent transition-opacity hover:opacity-70"
        >
          {action.label}
          <ChevronRightIcon className="h-3.5 w-3.5 opacity-80" />
        </button>
      )}
    </div>
  );
}

// Stack layout tuning.
const EXPANDED_GAP = 14;
const COLLAPSED_PEEK = 16;
const COLLAPSED_SCALE_STEP = 0.055;
const MAX_STACK = 2;

interface ToastCardProps {
  toast: ToastItem;
  paused: boolean;
  y: number;
  scale: number;
  hidden: boolean;
  contentVisible: boolean;
  zIndex: number;
  reportHeight: (id: string, height: number) => void;
}

function ToastCard({
  toast,
  paused,
  y,
  scale,
  hidden,
  contentVisible,
  zIndex,
  reportHeight
}: ToastCardProps) {
  const remove = useToastStore((s) => s.remove);
  const reduceMotion = useReducedMotion();
  const slideX = reduceMotion ? 0 : SLIDE_IN_X;
  const { icon: Icon, color, spin } = VARIANTS[toast.variant];
  const cardRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (cardRef.current) reportHeight(toast.id, cardRef.current.offsetHeight);
  });

  const startRef = useRef(0);
  const remainingRef = useRef(toast.duration);
  useEffect(() => {
    remainingRef.current = toast.duration;
  }, [toast.duration]);
  useEffect(() => {
    if (toast.duration === Infinity || paused) return;
    startRef.current = performance.now();
    const timer = window.setTimeout(() => remove(toast.id), Math.max(0, remainingRef.current));
    return () => {
      window.clearTimeout(timer);
      remainingRef.current -= performance.now() - startRef.current;
    };
  }, [paused, toast.id, toast.duration, remove]);

  return (
    <motion.div
      ref={cardRef}
      initial={{ opacity: 0, x: slideX, y, scale }}
      animate={{ opacity: hidden ? 0 : 1, x: 0, y, scale }}
      exit={{ opacity: 0, x: slideX, transition: { duration: 0.2 } }}
      transition={{ duration: 0.3, ease: [0.16, 1, 0.3, 1] }}
      style={{ zIndex, transformOrigin: "bottom right" }}
      className="pointer-events-auto absolute bottom-0 right-0 w-[356px] rounded-[10px] border border-edge bg-raised px-3 py-3 shadow-pop"
    >
      <motion.div
        animate={{ opacity: contentVisible ? 1 : 0 }}
        transition={{ duration: 0.15 }}
        className="flex flex-col gap-2"
      >
        <div className="flex items-center gap-2.5">
          <Icon
            className={`h-[18px] w-[18px] shrink-0 ${color} ${spin ? "animate-spin" : ""}`}
          />
          <div className="min-w-0 flex-1 text-[14px] font-semibold leading-tight text-ink">
            {toast.title}
          </div>
          <button
            type="button"
            aria-label="Dismiss"
            onClick={() => remove(toast.id)}
            className="toast-dismiss-btn shrink-0"
          >
            <XIcon className="h-3.5 w-3.5" />
          </button>
        </div>

        {toast.description &&
          (toast.copyable ? (
            <CopyableDescription description={toast.description} />
          ) : (
            <PlainDescription
              description={toast.description}
              action={toast.action}
              onDismiss={() => remove(toast.id)}
            />
          ))}
      </motion.div>
    </motion.div>
  );
}

/** Toast stack at bottom-right, portaled to <body> so it sits above every
 * stacking context. Collapsed by default; hovering expands into a full list. */
export function ToastViewport() {
  const toasts = useToastStore((s) => s.toasts);
  const remove = useToastStore((s) => s.remove);
  const [expanded, setExpanded] = useState(false);
  const [heights, setHeights] = useState<Record<string, number>>({});

  const reportHeight = useCallback((id: string, height: number) => {
    setHeights((prev) => (prev[id] === height ? prev : { ...prev, [id]: height }));
  }, []);

  useEffect(() => {
    if (toasts.length === 0) setExpanded(false);
  }, [toasts.length]);

  const ordered = [...toasts].reverse();
  const total = ordered.length;
  const stackHeight =
    ordered.reduce((sum, t) => sum + (heights[t.id] ?? 0), 0) +
    EXPANDED_GAP * Math.max(0, total - 1);
  const front = ordered[0];
  const frontHeight = front ? heights[front.id] ?? 0 : 0;
  const clearAllZone = total > 1 ? 44 : 0;
  const catcherHeight = expanded
    ? stackHeight + clearAllZone
    : frontHeight + MAX_STACK * COLLAPSED_PEEK;

  return createPortal(
    <div
      className="pointer-events-none fixed bottom-0 right-0 z-[9999] p-4"
      onMouseEnter={() => total > 0 && setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
    >
      <div className="relative h-0 w-[356px]">
        {total > 0 && (
          <div
            aria-hidden
            className="pointer-events-auto absolute bottom-0 right-0 z-0 w-[356px]"
            style={{ height: catcherHeight }}
          />
        )}
        <AnimatePresence>
          {expanded && total > 1 && (
            <motion.button
              key="__clear_all"
              type="button"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, y: -(stackHeight + 10) }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              onClick={() => remove()}
              className="toast-clear-btn pointer-events-auto absolute bottom-0 right-0 z-[200] rounded-md border border-edge bg-raised px-2.5 py-1 text-[11px] font-medium text-ink-muted shadow-card"
            >
              Clear all
            </motion.button>
          )}
        </AnimatePresence>

        <AnimatePresence initial={false}>
          {ordered.map((toast, index) => {
            const belowHeight = ordered
              .slice(0, index)
              .reduce((sum, t) => sum + (heights[t.id] ?? 0), 0);
            const y = expanded
              ? -(belowHeight + EXPANDED_GAP * index)
              : -(Math.min(index, MAX_STACK) * COLLAPSED_PEEK);
            const scale = expanded ? 1 : 1 - Math.min(index, MAX_STACK) * COLLAPSED_SCALE_STEP;
            return (
              <ToastCard
                key={toast.id}
                toast={toast}
                paused={expanded}
                y={y}
                scale={scale}
                hidden={!expanded && index > MAX_STACK}
                contentVisible={expanded || index === 0}
                zIndex={total - index}
                reportHeight={reportHeight}
              />
            );
          })}
        </AnimatePresence>
      </div>
    </div>,
    document.body
  );
}
