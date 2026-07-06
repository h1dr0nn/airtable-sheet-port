import type { ReactNode } from "react";

type ScreenHeaderProps = {
  title: string;
  description: string;
  actions?: ReactNode;
};

export function ScreenHeader({ title, description, actions }: ScreenHeaderProps) {
  return (
    <header className="mb-6 flex items-end justify-between gap-4">
      <div>
        <h2 className="text-xl font-semibold tracking-tight text-ink">{title}</h2>
        <p className="mt-1 text-[13px] text-ink-muted">{description}</p>
      </div>
      {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
    </header>
  );
}
