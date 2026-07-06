import { useRef, useState } from "react";
import { Button, useToast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";

const COPIED_RESET_MS = 1500;

type CopyButtonProps = {
  value: string;
  label: string;
};

export function CopyButton({ value, label }: CopyButtonProps) {
  const [isCopied, setIsCopied] = useState(false);
  const resetTimerRef = useRef<number | null>(null);
  const { toast } = useToast();

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setIsCopied(true);
      if (resetTimerRef.current !== null) {
        window.clearTimeout(resetTimerRef.current);
      }
      resetTimerRef.current = window.setTimeout(() => setIsCopied(false), COPIED_RESET_MS);
    } catch (error: unknown) {
      toast(getErrorMessage(error), "error");
    }
  };

  return (
    <Button
      variant="ghost"
      size="sm"
      aria-label={label}
      className="w-14 px-0"
      onClick={() => void handleCopy()}
    >
      {isCopied ? "[ OK ]" : "Copy"}
    </Button>
  );
}
