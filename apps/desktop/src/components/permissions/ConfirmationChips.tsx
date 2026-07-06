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

/** Square toggle markers; armed confirmation requirements read as hazard warnings. */
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
              "border px-2 py-0.5 font-mono text-[10px] uppercase tracking-[0.05em] transition-colors",
              "focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-hazard",
              "disabled:cursor-not-allowed disabled:opacity-40",
              isActive
                ? "border-hazard font-bold text-hazard"
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
