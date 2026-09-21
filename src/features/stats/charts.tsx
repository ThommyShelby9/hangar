import { useState, type ReactNode } from "react";
import { Link } from "react-router";
import { imageUrl, type StatsOverview } from "@/lib/api";
import { dayKey, formatDay, formatDuration } from "@/lib/format";
import { monthLabel, type Column } from "@/features/stats/columns";
import { cn } from "@/lib/utils";

// ── Hover tooltip ───────────────────────────────────────────────────────────

interface Tip {
  x: number;
  y: number;
  content: ReactNode;
}

function useTip() {
  const [tip, setTip] = useState<Tip | null>(null);
  const bind = (content: ReactNode) => ({
    onMouseEnter: (e: React.MouseEvent) => {
      const r = e.currentTarget.getBoundingClientRect();
      setTip({ x: r.left + r.width / 2, y: r.top, content });
    },
    onMouseLeave: () => setTip(null),
  });
  const node = tip && (
    <div
      role="tooltip"
      style={{ left: tip.x, top: tip.y }}
      className="pointer-events-none fixed z-50 -translate-x-1/2 -translate-y-[calc(100%+8px)] rounded-control border border-rivet bg-bay px-3 py-2 text-caption whitespace-nowrap shadow-lg shadow-black/40"
    >
      {tip.content}
    </div>
  );
  return { bind, node };
}

// ── Columns: play time per day or per month ─────────────────────────────────

/** Column chart, one series: play time. Label only the busiest column; the rest is in the tooltip. */
export function PlayColumns({ columns }: { columns: Column[] }) {
  const { bind, node } = useTip();
  const max = Math.max(...columns.map((c) => c.secs), 1);
  const peak = columns.reduce((a, b) => (b.secs > a.secs ? b : a), columns[0]);
  const everyNth = columns.length > 14 ? 5 : 1;

  return (
    <figure className="relative">
      <div className="absolute inset-x-0 top-0 border-t border-rivet/70" aria-hidden />
      <div className="flex h-44 items-end gap-0.5 border-b border-rivet">
        {columns.map((c, i) => (
          <div
            key={c.key}
            {...bind(
              <>
                <span className="block text-alu-dim first-letter:uppercase">{c.title}</span>
                <span className="font-medium">{c.secs ? formatDuration(c.secs) : "Pas joué"}</span>
              </>,
            )}
            className="group relative flex h-full flex-1 items-end justify-center"
          >
            {c === peak && c.secs > 0 && (
              <span
                className="absolute text-caption whitespace-nowrap text-alu-dim"
                style={{ bottom: `calc(${(c.secs / max) * 100}% + 4px)` }}
              >
                {formatDuration(c.secs)}
              </span>
            )}
            <div
              className="w-full max-w-6 rounded-t-[4px] bg-signal transition-opacity group-hover:opacity-80"
              style={{ height: c.secs ? `max(2px, ${(c.secs / max) * 100}%)` : 0 }}
            />
            <span className="sr-only">
              {c.title} : {formatDuration(c.secs)}
            </span>
            {i % everyNth === 0 && (
              <span className="absolute -bottom-6 text-caption text-alu-dim tabular-nums first-letter:uppercase">
                {c.label}
              </span>
            )}
          </div>
        ))}
      </div>
      <div className="h-6" />
      {node}
    </figure>
  );
}

// ── Calendar heatmap: the last 12 months ────────────────────────────────────

/** Sequential scale on the amber hue: more play time, more light. */
const LEVELS = [
  { upTo: 0, className: "bg-rivet/45", label: "Pas joué" },
  { upTo: 30 * 60, className: "bg-signal/25", label: "Moins de 30 min" },
  { upTo: 60 * 60, className: "bg-signal/45", label: "30 min à 1 h" },
  { upTo: 2 * 3600, className: "bg-signal/70", label: "1 h à 2 h" },
  { upTo: Infinity, className: "bg-signal", label: "Plus de 2 h" },
];

const level = (secs: number) => LEVELS.find((l) => secs <= l.upTo)!;

export function PlayCalendar({ daily }: { daily: StatsOverview["daily"] }) {
  const { bind, node } = useTip();
  const byDay = new Map(daily.map((d) => [d.day, d.secs]));
  const today = new Date();
  // Start on the Monday 52 weeks back, so each column is one week, Monday on top.
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 364);
  start.setDate(start.getDate() - ((start.getDay() + 6) % 7));

  const weeks: Date[][] = [];
  for (const d = new Date(start); d <= today; d.setDate(d.getDate() + 1)) {
    if ((d.getDay() + 6) % 7 === 0) weeks.push([]);
    weeks[weeks.length - 1].push(new Date(d));
  }

  return (
    <figure>
      <div className="overflow-x-auto pb-1">
        <div className="inline-flex gap-[3px]">
          {weeks.map((week, w) => {
            const firstOfMonth = week.find((d) => d.getDate() === 1);
            return (
              <div key={w} className="relative flex flex-col gap-[3px] pt-5">
                {firstOfMonth && (
                  <span className="absolute top-0 left-0 text-caption text-alu-dim">
                    {monthLabel.format(firstOfMonth).replace(".", "")}
                  </span>
                )}
                {week.map((d) => {
                  const key = dayKey(d);
                  const secs = byDay.get(key) ?? 0;
                  return (
                    <div
                      key={key}
                      {...bind(
                        <>
                          <span className="block text-alu-dim first-letter:uppercase">
                            {formatDay(key)}
                          </span>
                          <span className="font-medium">
                            {secs ? formatDuration(secs) : "Pas joué"}
                          </span>
                        </>,
                      )}
                      aria-label={`${formatDay(key)} : ${secs ? formatDuration(secs) : "pas joué"}`}
                      className={cn("size-3 rounded-[2px]", level(secs).className)}
                    />
                  );
                })}
              </div>
            );
          })}
        </div>
      </div>
      <figcaption className="mt-3 flex items-center gap-1.5 text-caption text-alu-dim">
        Moins
        {LEVELS.map((l) => (
          <span key={l.label} title={l.label} className={cn("size-3 rounded-[2px]", l.className)} />
        ))}
        Plus
      </figcaption>
      {node}
    </figure>
  );
}

// ── Most played games: horizontal bars, value at the tip ────────────────────

export function TopGames({ games }: { games: StatsOverview["topGames"] }) {
  const max = Math.max(...games.map((g) => g.secs), 1);
  return (
    <ol className="space-y-3">
      {games.map((g) => (
        <li key={g.gameId}>
          <Link
            to={`/jeu/${g.gameId}`}
            className="group grid grid-cols-[2rem_minmax(8rem,14rem)_1fr] items-center gap-4 rounded-control"
          >
            {g.grid ? (
              <img
                src={imageUrl(g.grid)}
                alt=""
                className="aspect-[2/3] w-8 rounded-[3px] object-cover"
              />
            ) : (
              <span className="aspect-[2/3] w-8 rounded-[3px] bg-bay" />
            )}
            <span className="truncate group-hover:text-alu">{g.title}</span>
            <span className="flex items-center gap-3">
              <span
                className="h-3 rounded-r-[4px] bg-signal"
                style={{ width: `max(4px, ${(g.secs / max) * 85}%)` }}
              />
              <span className="shrink-0 text-alu-dim tabular-nums">{formatDuration(g.secs)}</span>
            </span>
          </Link>
        </li>
      ))}
    </ol>
  );
}
