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
  success: "border-accent/30 text-accent",
  error: "border-danger/30 text-danger",
  info: "border-info/30 text-info"
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
                "pointer-events-auto rounded-md border bg-raised px-3.5 py-2.5 text-[13px] font-medium",
                "shadow-raised motion-safe:animate-toast-in",
                VARIANT_CLASSES[item.variant]
              )}
            >
              {item.message}
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
