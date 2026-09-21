import type { ReactNode } from "react";

/** Centered message for a section that has nothing to show yet. */
export function EmptyState({
  title,
  children,
  art,
}: {
  title: string;
  children: ReactNode;
  art?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center px-10 pt-10 pb-16 text-center">
      {art}
      <h2 className="font-display text-title font-semibold text-alu">{title}</h2>
      <p className="mt-2 max-w-[52ch] leading-relaxed text-alu-dim">{children}</p>
    </div>
  );
}
