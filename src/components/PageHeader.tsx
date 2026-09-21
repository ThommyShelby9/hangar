import type { ReactNode } from "react";

export function PageHeader({
  title,
  children,
  actions,
}: {
  title: string;
  children?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <header className="flex flex-wrap items-end justify-between gap-x-8 gap-y-4 px-10 pt-9 pb-6">
      <div>
        <h1 className="font-display text-page font-bold tracking-[-0.01em] text-alu">{title}</h1>
        {children && <p className="mt-3 max-w-[60ch] text-lead text-alu-dim">{children}</p>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </header>
  );
}
