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

// Variant reads as a colored left border; the body stays neutral.
const VARIANT_CLASSES: Record<ToastVariant, string> = {
  success: "border-l-success",
  error: "border-l-danger",
  info: "border-l-edge-strong"
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
                "pointer-events-auto rounded-lg border border-edge bg-raised px-4 py-3 shadow-pop",
                "border-l-2 font-sans text-[13px] leading-5 text-ink",
                "motion-safe:animate-fade-in",
                VARIANT_CLASSES[item.variant]
              )}
            >
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
