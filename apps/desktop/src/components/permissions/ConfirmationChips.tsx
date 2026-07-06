import { cn } from "@sheet-port/ui";
import type { ConfirmationAction } from "@sheet-port/shared";

export const CONFIRMATION_ACTIONS: readonly ConfirmationAction[] = [
  "append",
  "update",
  "delete",
  "bulk_update",
  "formula_change"
];

type ConfirmationChipsProps = {
  value: readonly string[];
  onChange: (next: string[]) => void;
  disabled?: boolean;
};

/** Toggleable chips for the actions that require user confirmation. */
export function ConfirmationChips({ value, onChange, disabled = false }: ConfirmationChipsProps) {
  const toggle = (action: ConfirmationAction) => {
    const next = value.includes(action)
      ? value.filter((item) => item !== action)
      : [...value, action];
    onChange(next);
  };

  return (
    <div className="flex flex-wrap gap-1.5" role="group" aria-label="Actions requiring confirmation">
      {CONFIRMATION_ACTIONS.map((action) => {
        const isActive = value.includes(action);
        return (
          <button
            key={action}
            type="button"
            aria-pressed={isActive}
            disabled={disabled}
            onClick={() => toggle(action)}
            className={cn(
              "rounded-full border px-2.5 py-0.5 font-mono text-[11px] transition-colors",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60",
              "disabled:cursor-not-allowed disabled:opacity-45",
              isActive
                ? "border-warning/30 bg-warning/10 text-warning hover:bg-warning/15"
                : "border-edge text-ink-muted hover:border-edge-strong hover:text-ink"
            )}
          >
            {action}
          </button>
        );
      })}
    </div>
  );
}
