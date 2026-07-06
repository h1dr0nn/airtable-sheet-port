import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode
} from "react";
import { createPortal } from "react-dom";
import { cn } from "./cn.js";

export type ToastVariant = "success" | "error" | "info";

type ToastItem = {
  id: number;
  message: string;
  variant: ToastVariant;
};

type ToastContextValue = {
  toast: (message: string, variant?: ToastVariant) => void;
};

const ToastContext = createContext<ToastContextValue | null>(null);

const TOAST_DURATION_MS = 4000;

const VARIANT_CLASSES: Record<ToastVariant, string> = {
  success: "border-ink text-ink",
  error: "border-hazard text-hazard",
  info: "border-edge-strong text-ink"
};

const VARIANT_MARKERS: Record<ToastVariant, string> = {
  success: "[ OK ]",
  error: "[ ERR ]",
  info: "[ SYS ]"
};

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<readonly ToastItem[]>([]);
  const nextIdRef = useRef(0);

  const toast = useCallback((message: string, variant: ToastVariant = "info") => {
    const id = nextIdRef.current;
    nextIdRef.current += 1;
    setItems((current) => [...current, { id, message, variant }]);
    window.setTimeout(() => {
      setItems((current) => current.filter((item) => item.id !== id));
    }, TOAST_DURATION_MS);
  }, []);

  const value = useMemo(() => ({ toast }), [toast]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      {createPortal(
        <div className="pointer-events-none fixed bottom-4 right-4 z-[100] flex w-80 flex-col gap-2">
          {items.map((item) => (
            <div
              key={item.id}
              role="status"
              className={cn(
                "pointer-events-auto flex items-baseline gap-2 border bg-raised px-3.5 py-2.5",
                "font-mono text-[11px] uppercase tracking-[0.05em]",
                "motion-safe:animate-fade-in",
                VARIANT_CLASSES[item.variant]
              )}
            >
              <span className="shrink-0 font-bold" aria-hidden>
                {VARIANT_MARKERS[item.variant]}
              </span>
              <span className="min-w-0 break-words">{item.message}</span>
            </div>
          ))}
        </div>,
        document.body
      )}
    </ToastContext.Provider>
  );
}

export function useToast(): ToastContextValue {
  const context = useContext(ToastContext);
  if (context === null) {
    throw new Error("useToast must be used within a ToastProvider");
  }
  return context;
}
