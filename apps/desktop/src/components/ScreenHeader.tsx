import type { ReactNode } from "react";

const DEFAULT_META = "UNIT / D-01";

type ScreenHeaderProps = {
  title: string;
  description: string;
  /** Right-aligned mono metadata string on the rule, e.g. "SRC 3 / RULES 1". */
  meta?: string;
  actions?: ReactNode;
};

/** Macro title block: display-face headline over a 2px rule with telemetry metadata. */
export function ScreenHeader({ title, description, meta, actions }: ScreenHeaderProps) {
  return (
    <header className="mb-8">
      <div className="flex flex-wrap items-end justify-between gap-x-6 gap-y-4">
        <h2 className="font-display text-[clamp(2rem,4vw,3.5rem)] uppercase leading-[0.9] tracking-[-0.02em] text-ink">
          {title}
        </h2>
        {actions ? <div className="flex shrink-0 items-center gap-2 pb-1">{actions}</div> : null}
      </div>
      <div className="mt-5 flex items-baseline justify-between gap-4 border-t-2 border-ink pt-2">
        <p className="font-mono text-[11px] uppercase tracking-[0.08em] text-ink-muted">{description}</p>
        <p className="shrink-0 font-mono text-[10px] uppercase tracking-[0.08em] text-ink-muted">
          {meta ?? DEFAULT_META}
        </p>
      </div>
    </header>
  );
}
