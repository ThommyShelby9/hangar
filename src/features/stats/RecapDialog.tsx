import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  Clock,
  Moon,
  Sparkles,
  Sun,
  Sunrise,
  Sunset,
  TrendingDown,
  TrendingUp,
  X,
} from "lucide-react";
import { imageUrl, statsApi } from "@/lib/api";
import { formatDuration, formatShortDate } from "@/lib/format";
import { buildRecap, monthBounds } from "@/features/stats/recap";

const MONTH = new Intl.DateTimeFormat("fr-FR", { month: "long", year: "numeric" });
const WEEKDAYS = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
const PERSONA_ICON = {
  "Oiseau de nuit": Moon,
  "Lève-tôt": Sunrise,
  "Joueur de l'après-midi": Sun,
  "Joueur du soir": Sunset,
};

/** "Rétro du mois": a recap of one month of play, in the spirit of year-end "Wrapped" pages. */
export function RecapDialog({ onClose }: { onClose: () => void }) {
  const now = new Date();
  const [month, setMonth] = useState({ year: now.getFullYear(), month: now.getMonth() });
  const current = monthBounds(month.year, month.month);
  const previous = monthBounds(month.year, month.month - 1);
  const isCurrentMonth = month.year === now.getFullYear() && month.month === now.getMonth();

  const { data: recap } = useQuery({
    queryKey: ["stats", "recap", month.year, month.month],
    queryFn: async () => {
      const [sessions, before] = await Promise.all([
        statsApi.sessionsBetween(current.from, current.to),
        statsApi.sessionsBetween(previous.from, previous.to),
      ]);
      const previousSecs = before.reduce((sum, s) => sum + s.durationSecs, 0);
      return buildRecap(sessions, current.days, previousSecs || null);
    },
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const shift = (delta: number) => {
    const d = new Date(month.year, month.month + delta, 1);
    setMonth({ year: d.getFullYear(), month: d.getMonth() });
  };
  const top = recap?.games[0];
  const PersonaIcon = recap
    ? (PERSONA_ICON[recap.persona.name as keyof typeof PERSONA_ICON] ?? Clock)
    : Clock;

  return createPortal(
    <motion.div
      role="dialog"
      aria-modal="true"
      aria-label="Rétro du mois"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      className="fixed inset-0 z-50 overflow-y-auto bg-tarmac/95 backdrop-blur-md"
    >
      <div className="mx-auto max-w-5xl px-10 py-10">
        <header className="flex items-center gap-4">
          <button
            aria-label="Mois précédent"
            onClick={() => shift(-1)}
            className="rounded-full p-2 text-alu-dim hover:bg-rivet/40 hover:text-alu"
          >
            <ChevronLeft size={24} />
          </button>
          <h2 className="font-display text-page font-bold first-letter:uppercase">
            {MONTH.format(new Date(month.year, month.month, 1))}
          </h2>
          <button
            aria-label="Mois suivant"
            disabled={isCurrentMonth}
            onClick={() => shift(1)}
            className="rounded-full p-2 text-alu-dim hover:bg-rivet/40 hover:text-alu disabled:invisible"
          >
            <ChevronRight size={24} />
          </button>
          <span className="ml-2 text-alu-dim">
            {isCurrentMonth ? "Rétro du mois en cours" : "Rétro du mois"}
          </span>
          <button
            aria-label="Fermer"
            onClick={onClose}
            className="ml-auto rounded-full p-2 text-alu-dim hover:bg-rivet/40 hover:text-alu"
          >
            <X size={22} />
          </button>
        </header>

        {!recap ? null : recap.sessions === 0 ? (
          <p className="mt-16 text-center text-lead text-alu-dim">
            Aucune partie enregistrée ce mois-là.
          </p>
        ) : (
          <motion.div
            key={`${month.year}-${month.month}`}
            initial="hidden"
            animate="shown"
            variants={{ shown: { transition: { staggerChildren: 0.08 } } }}
            className="mt-8 grid gap-4 md:grid-cols-3"
          >
            {top && (
              <Tile className="md:col-span-2 md:row-span-2">
                <div className="flex h-full gap-6">
                  {top.grid && (
                    <img
                      src={imageUrl(top.grid)}
                      alt=""
                      className="aspect-[2/3] w-40 shrink-0 rounded-[8px] object-cover shadow-2xl shadow-black/60"
                    />
                  )}
                  <div className="flex flex-col justify-end">
                    <p className="text-caption tracking-[0.18em] text-signal uppercase">
                      Jeu du mois
                    </p>
                    <p className="mt-2 font-display text-[2.5rem] leading-none font-bold">
                      {top.title}
                    </p>
                    <p className="mt-3 text-lead text-alu-dim">
                      {formatDuration(top.secs)} en {top.sessions} session
                      {top.sessions > 1 ? "s" : ""}, soit{" "}
                      {Math.round((top.secs / recap.totalSecs) * 100)} % de ton temps de jeu.
                    </p>
                  </div>
                </div>
              </Tile>
            )}
            <Tile>
              <Label icon={Clock}>Temps de jeu</Label>
              <Big>{formatDuration(recap.totalSecs)}</Big>
              {recap.trend !== null && (
                <p className={recap.trend >= 0 ? "text-primer" : "text-alu-dim"}>
                  {recap.trend >= 0 ? (
                    <TrendingUp size={15} className="mr-1 inline" />
                  ) : (
                    <TrendingDown size={15} className="mr-1 inline" />
                  )}
                  {recap.trend >= 0 ? "+" : ""}
                  {recap.trend} % par rapport au mois d'avant
                </p>
              )}
            </Tile>
            <Tile>
              <Label icon={CalendarDays}>Jours de jeu</Label>
              <Big>
                {recap.daysPlayed}
                <span className="text-title text-alu-dim"> / {recap.daysInMonth}</span>
              </Big>
              {recap.bestWeekday !== null && (
                <p className="text-alu-dim">Ton jour préféré : le {WEEKDAYS[recap.bestWeekday]}</p>
              )}
            </Tile>
            <Tile>
              <Label icon={PersonaIcon}>Ton profil</Label>
              <Big>{recap.persona.name}</Big>
              <p className="text-alu-dim">{recap.persona.detail}</p>
            </Tile>
            {recap.longest && (
              <Tile>
                <Label icon={Clock}>Plus longue session</Label>
                <Big>{formatDuration(recap.longest.durationSecs)}</Big>
                <p className="text-alu-dim">
                  {recap.longest.title}, le {formatShortDate(recap.longest.startedAt)}
                </p>
              </Tile>
            )}
            <Tile>
              <Label icon={Sparkles}>Découvertes</Label>
              {recap.discoveries.length ? (
                <>
                  <Big>{recap.discoveries.length}</Big>
                  <p className="text-alu-dim">{recap.discoveries.join(", ")}</p>
                </>
              ) : (
                <p className="mt-2 text-alu-dim">
                  Pas de nouveau jeu : tu es resté fidèle à tes classiques.
                </p>
              )}
            </Tile>
            {recap.games.length > 1 && (
              <Tile className="md:col-span-3">
                <Label icon={Sparkles}>Le classement</Label>
                <ol className="mt-3 grid gap-x-8 gap-y-2 sm:grid-cols-2">
                  {recap.games.slice(0, 6).map((g, i) => (
                    <li key={g.gameId} className="flex items-baseline gap-3">
                      <span className="w-5 font-display text-title font-bold text-signal tabular-nums">
                        {i + 1}
                      </span>
                      <span className="flex-1 truncate">{g.title}</span>
                      <span className="text-alu-dim tabular-nums">{formatDuration(g.secs)}</span>
                    </li>
                  ))}
                </ol>
              </Tile>
            )}
          </motion.div>
        )}
      </div>
    </motion.div>,
    document.body,
  );
}

function Tile({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <motion.section
      variants={{ hidden: { opacity: 0, y: 14 }, shown: { opacity: 1, y: 0 } }}
      className={`rounded-panel border border-rivet bg-bay p-6 ${className ?? ""}`}
    >
      {children}
    </motion.section>
  );
}

function Label({ icon: Icon, children }: { icon: typeof Clock; children: ReactNode }) {
  return (
    <p className="flex items-center gap-2 text-caption tracking-[0.12em] text-alu-dim uppercase">
      <Icon size={14} /> {children}
    </p>
  );
}

function Big({ children }: { children: ReactNode }) {
  return <p className="mt-2 mb-1 font-display text-[2.25rem] leading-none font-bold">{children}</p>;
}
