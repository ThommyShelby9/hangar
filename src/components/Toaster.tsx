import { AnimatePresence, motion } from "motion/react";
import { X } from "lucide-react";
import { useToasts } from "@/lib/toast";
import { cn } from "@/lib/utils";

export function Toaster() {
  const { toasts, dismiss } = useToasts();
  return (
    <div className="pointer-events-none fixed right-5 bottom-5 z-50 flex w-96 flex-col gap-2">
      <AnimatePresence initial={false}>
        {toasts.map((t) => (
          <motion.div
            key={t.id}
            layout
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, x: 24 }}
            role={t.tone === "error" ? "alert" : "status"}
            className={cn(
              "pointer-events-auto flex items-start gap-3 rounded-control border bg-bay py-3 pr-2 pl-4 shadow-lg shadow-black/30",
              t.tone === "error" ? "border-flare/60" : "border-rivet",
            )}
          >
            <p className="flex-1 leading-snug select-text">{t.message}</p>
            <button
              type="button"
              aria-label="Fermer"
              onClick={() => dismiss(t.id)}
              className="rounded p-1 text-alu-dim hover:text-alu"
            >
              <X size={14} />
            </button>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
