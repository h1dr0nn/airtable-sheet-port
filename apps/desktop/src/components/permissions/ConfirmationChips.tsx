import { cn, FOCUS_RING } from "@sheet-port/ui";
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

/** Toggle chips; armed confirmation requirements read as amber markers. */
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
              "rounded-full border px-2.5 py-1 text-[12px] font-medium transition-colors",
              FOCUS_RING,
              "disabled:cursor-not-allowed disabled:opacity-50",
              isActive
                ? "border-warning/40 bg-warning/10 text-warning"
                : "border-edge text-ink-muted hover:bg-surface hover:text-ink"
            )}
          >
            {action}
          </button>
        );
      })}
    </div>
  );
}
