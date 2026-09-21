import type { ReactNode } from "react";

export function PageHeader({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <header className="px-10 pt-9 pb-6">
      <h1 className="font-display text-page font-bold tracking-[-0.01em] text-alu">{title}</h1>
      {children && <p className="mt-3 max-w-[60ch] text-lead text-alu-dim">{children}</p>}
    </header>
  );
}
