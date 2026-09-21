import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { listen } from "@tauri-apps/api/event";
import { X } from "lucide-react";
import { SESSION_ENDED, type SessionEnd } from "@/lib/api";
import { formatDuration } from "@/lib/format";

const SHOWN_FOR_MS = 30_000;

/** Card shown when a game closes: how long it lasted and how hot the PC ran. */
export function SessionSummary() {
  const [session, setSession] = useState<SessionEnd | null>(null);

  useEffect(() => {
    const off = listen<SessionEnd>(SESSION_ENDED, (e) => setSession(e.payload));
    return () => {
      off.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (!session) return;
    const timer = setTimeout(() => setSession(null), SHOWN_FOR_MS);
    return () => clearTimeout(timer);
  }, [session]);

  return (
    <AnimatePresence>
      {session && (
        <motion.aside
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 16 }}
          role="status"
          className="fixed bottom-5 left-60 z-50 w-[22rem] rounded-panel border border-rivet bg-bay p-5 shadow-2xl shadow-black/50"
        >
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-caption text-alu-dim">Session terminée</p>
              <p className="font-display text-title leading-tight font-semibold">{session.title}</p>
            </div>
            <button
              type="button"
              aria-label="Fermer le résumé"
              onClick={() => setSession(null)}
              className="rounded p-1 text-alu-dim hover:text-alu"
            >
              <X size={16} />
            </button>
          </div>
          <dl className="mt-4 grid grid-cols-2 gap-x-4 gap-y-3">
            <Stat label="Durée" value={formatDuration(session.durationSecs)} />
            <Stat
              label="Processeur, charge moyenne"
              value={session.avgCpuLoad !== null ? `${Math.round(session.avgCpuLoad)} %` : "—"}
            />
            <Stat
              label="Processeur, max"
              value={session.maxCpuTemp !== null ? `${Math.round(session.maxCpuTemp)} °C` : "—"}
            />
            <Stat
              label="Carte graphique, max"
              value={session.maxGpuTemp !== null ? `${Math.round(session.maxGpuTemp)} °C` : "—"}
            />
          </dl>
        </motion.aside>
      )}
    </AnimatePresence>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-caption text-alu-dim">{label}</dt>
      <dd className="text-[1.25rem] font-semibold tabular-nums">{value}</dd>
    </div>
  );
}
