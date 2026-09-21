import { useEffect, useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";

/** Native <dialog>: focus trap and Esc handling come from the browser. */
export function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  className,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  className?: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      onClose={onClose}
      onClick={(e) => e.target === ref.current && onClose()}
      className={cn(
        "m-auto max-h-[85vh] w-[min(46rem,90vw)] flex-col overflow-hidden rounded-panel border border-rivet bg-tarmac p-0 text-alu shadow-2xl shadow-black/50 backdrop:bg-black/55 open:flex",
        className,
      )}
    >
      <header className="flex items-center justify-between border-b border-rivet py-3 pr-3 pl-6">
        <h2 className="font-display text-title font-semibold">{title}</h2>
        <button
          type="button"
          aria-label="Fermer"
          onClick={onClose}
          className="rounded p-1.5 text-alu-dim hover:text-alu"
        >
          <X size={18} />
        </button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
      {footer && (
        <footer className="flex items-center justify-end gap-3 border-t border-rivet px-6 py-3">
          {footer}
        </footer>
      )}
    </dialog>
  );
}
