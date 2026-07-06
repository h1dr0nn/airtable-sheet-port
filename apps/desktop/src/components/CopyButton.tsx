import { useRef, useState, type ReactNode } from "react";
import { Button, toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";

const COPIED_RESET_MS = 1500;

type CopyButtonProps = {
  value: string;
  /** Accessible label describing what gets copied. */
  label: string;
  /** Visible idle text; defaults to "Copy". */
  children?: ReactNode;
};

export function CopyButton({ value, label, children }: CopyButtonProps) {
  const [isCopied, setIsCopied] = useState(false);
  const resetTimerRef = useRef<number | null>(null);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setIsCopied(true);
      if (resetTimerRef.current !== null) {
        window.clearTimeout(resetTimerRef.current);
      }
      resetTimerRef.current = window.setTimeout(() => setIsCopied(false), COPIED_RESET_MS);
    } catch (error: unknown) {
      toast.error("Copy failed", { description: getErrorMessage(error) });
    }
  };

  return (
    <Button
      variant="ghost"
      size="sm"
      aria-label={label}
      className="min-w-16 px-2"
      onClick={() => void handleCopy()}
    >
      {isCopied ? "Copied" : children ?? "Copy"}
    </Button>
  );
}
