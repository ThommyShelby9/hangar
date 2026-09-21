import { useState, type ReactNode } from "react";
import { Link } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { statsApi } from "@/lib/api";
import { formatDateTime, formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/Button";
import { Sparkles } from "lucide-react";
import { RecapDialog } from "@/features/stats/RecapDialog";
import { EmptyState } from "@/components/EmptyState";
import { PlayCalendar, PlayColumns, TopGames } from "@/features/stats/charts";
import { buildColumns } from "@/features/stats/columns";

const PERIODS = [
  { days: 7, label: "7 jours" },
  { days: 30, label: "30 jours" },
  { days: 365, label: "12 mois" },
  { days: null, label: "Tout" },
] as const;

type Days = (typeof PERIODS)[number]["days"];

export function StatsPage() {
  const [days, setDays] = useState<Days>(30);
  const [recap, setRecap] = useState(false);
  const { data } = useQuery({
    queryKey: ["stats", "overview", days],
    queryFn: () => statsApi.overview(days),
    refetchInterval: 60_000,
  });
  const { data: year } = useQuery({
    queryKey: ["stats", "overview", "calendar"],
    queryFn: () => statsApi.overview(371),
  });
  const { data: allTime } = useQuery({
    queryKey: ["stats", "overview", null],
    queryFn: () => statsApi.overview(null),
  });

  if (allTime && allTime.sessionCount === 0) {
    return (
      <>
        <PageHeader title="Stats" />
        <EmptyState title="Aucune session enregistrée">
          Le temps passé sur chaque jeu s'affichera ici dès ta première partie, même si tu lances le
          jeu sans passer par Hangar. Les parties de moins d'une minute ne comptent pas.
        </EmptyState>
      </>
    );
  }

  return (
    <>
      <PageHeader
        title="Stats"
        actions={
          <>
            <Button variant="primary" onClick={() => setRecap(true)}>
              <Sparkles size={16} /> Rétro du mois
            </Button>
            <div role="radiogroup" aria-label="Période" className="flex gap-1">
              {PERIODS.map((p) => (
                <button
                  key={p.label}
                  role="radio"
                  aria-checked={days === p.days}
                  onClick={() => setDays(p.days)}
                  className={cn(
                    "h-9 rounded-control px-3.5 transition-colors",
                    days === p.days
                      ? "bg-rivet/60 text-alu"
                      : "text-alu-dim hover:bg-rivet/30 hover:text-alu",
                  )}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </>
        }
      />
      {recap && <RecapDialog onClose={() => setRecap(false)} />}

      {data && (
        <div className="space-y-12 px-10 pb-12">
          <section className="flex flex-wrap items-end gap-x-14 gap-y-6">
            <div>
              <p className="text-alu-dim">Temps de jeu</p>
              <p className="mt-1 text-[3.5rem] leading-none font-semibold">
                {formatDuration(data.totalSecs)}
              </p>
            </div>
            <Figure label="Sessions" value={data.sessionCount} />
            <Figure label="Jeux joués" value={data.gamesPlayed} />
          </section>

          {data.sessionCount === 0 ? (
            <p className="text-alu-dim">Aucune partie sur cette période.</p>
          ) : (
            <>
              <Section title={days !== null && days <= 31 ? "Par jour" : "Par mois"}>
                <PlayColumns columns={buildColumns(data.daily, days)} />
              </Section>

              <Section title="Les plus joués">
                <TopGames games={data.topGames} />
              </Section>
            </>
          )}

          {year && (
            <Section title="Les 12 derniers mois">
              <PlayCalendar daily={year.daily} />
            </Section>
          )}

          {data.recent.length > 0 && (
            <Section title="Dernières sessions">
              <table className="w-full max-w-3xl text-left">
                <thead className="text-caption text-alu-dim">
                  <tr className="border-b border-rivet">
                    <th className="py-2 font-normal">Jeu</th>
                    <th className="py-2 font-normal">Début</th>
                    <th className="py-2 text-right font-normal">Processeur max</th>
                    <th className="py-2 text-right font-normal">Durée</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-rivet/60">
                  {data.recent.map((s) => (
                    <tr key={s.id}>
                      <td className="py-2.5">
                        <Link to={`/jeu/${s.gameId}`} className="hover:text-signal">
                          {s.title}
                        </Link>
                      </td>
                      <td className="py-2.5 text-alu-dim">{formatDateTime(s.startedAt)}</td>
                      <td className="py-2.5 text-right text-alu-dim tabular-nums">
                        {s.maxCpuTemp !== null ? `${Math.round(s.maxCpuTemp)} °C` : "—"}
                      </td>
                      <td className="py-2.5 text-right tabular-nums">
                        {s.running ? (
                          <span className="inline-flex items-center gap-2 text-primer">
                            <span className="size-2 rounded-full bg-primer" />
                            En cours · {formatDuration(s.durationSecs)}
                          </span>
                        ) : (
                          formatDuration(s.durationSecs)
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </Section>
          )}
        </div>
      )}
    </>
  );
}

function Figure({ label, value }: { label: string; value: number }) {
  return (
    <div>
      <p className="text-alu-dim">{label}</p>
      <p className="mt-1 text-[2rem] leading-none font-semibold">{value.toLocaleString("fr-FR")}</p>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="mb-5 font-display text-title font-semibold">{title}</h2>
      {children}
    </section>
  );
}
