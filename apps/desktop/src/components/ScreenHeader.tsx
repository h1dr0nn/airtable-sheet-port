import type { ReactNode } from "react";

type ScreenHeaderProps = {
  title: string;
  description: string;
  actions?: ReactNode;
};

/** Screen title block: quiet hierarchy from size and weight, not shouting. */
export function ScreenHeader({ title, description, actions }: ScreenHeaderProps) {
  return (
    <header className="mb-7 flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
      <div className="min-w-0">
        <h2 className="text-[23px] font-semibold leading-tight tracking-[-0.01em] text-ink">{title}</h2>
        <p className="mt-1 text-[13px] text-ink-muted">{description}</p>
      </div>
      {actions ? <div className="flex shrink-0 items-center gap-2 pt-1">{actions}</div> : null}
    </header>
  );
}
