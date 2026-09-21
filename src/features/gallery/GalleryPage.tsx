import { useMemo, useState } from "react";
import { useSearchParams } from "react-router";
import { Camera } from "lucide-react";
import type { Shot } from "@/lib/api";
import { PageHeader } from "@/components/PageHeader";
import { EmptyState } from "@/components/EmptyState";
import { cn } from "@/lib/utils";
import { useGames } from "@/features/library/queries";
import { useShots } from "@/features/gallery/queries";
import { ShotGrid } from "@/features/gallery/ShotGrid";
import { Lightbox } from "@/features/gallery/Lightbox";

const OUTSIDE = "autres";

export function GalleryPage() {
  const { data: shots, isLoading } = useShots();
  const { data: games } = useGames();
  const [params, setParams] = useSearchParams();
  const filter = params.get("jeu");

  const titles = useMemo(() => new Map(games?.map((g) => [g.id, g.title])), [games]);
  const titleOf = (shot: Shot) =>
    shot.gameId === null ? "Hors bibliothèque" : (titles.get(shot.gameId) ?? "Jeu retiré");

  const chips = useMemo(() => {
    const counts = new Map<string, number>();
    for (const shot of shots ?? []) {
      const key = shot.gameId === null ? OUTSIDE : String(shot.gameId);
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return [...counts]
      .map(([key, count]) => ({
        key,
        count,
        label: key === OUTSIDE ? "Hors bibliothèque" : (titles.get(Number(key)) ?? "Jeu retiré"),
      }))
      .sort((a, b) => (a.key === OUTSIDE ? 1 : b.key === OUTSIDE ? -1 : b.count - a.count));
  }, [shots, titles]);

  const visible = (shots ?? []).filter(
    (s) => !filter || (filter === OUTSIDE ? s.gameId === null : String(s.gameId) === filter),
  );
  const setFilter = (key: string | null) => setParams(key ? { jeu: key } : {}, { replace: true });

  return (
    <div className="pb-12">
      <PageHeader title="Captures">
        Tes captures de la Game Bar (Win + Alt + Impr. écran), de Steam et de Windows, rangées par jeu
        grâce à l'heure de tes sessions.
      </PageHeader>

      {!isLoading && !shots?.length ? (
        <EmptyState
          title="Aucune capture pour l'instant"
          art={<Camera size={40} className="mb-4 text-alu-dim" />}
        >
          En jeu, appuie sur Win + Alt + Impr. écran (ou F12 dans Steam) : la capture apparaîtra ici,
          sous le bon jeu.
        </EmptyState>
      ) : (
        <>
          <div className="flex flex-wrap gap-2 px-10 pb-6">
            <Chip active={!filter} onClick={() => setFilter(null)}>
              Toutes <Count>{shots?.length ?? 0}</Count>
            </Chip>
            {chips.map((chip) => (
              <Chip key={chip.key} active={filter === chip.key} onClick={() => setFilter(chip.key)}>
                {chip.label} <Count>{chip.count}</Count>
              </Chip>
            ))}
          </div>
          <ByMonth shots={visible} titleOf={titleOf} showTitles={!filter} />
        </>
      )}
    </div>
  );
}

function ByMonth({
  shots,
  titleOf,
  showTitles,
}: {
  shots: Shot[];
  titleOf: (shot: Shot) => string;
  showTitles: boolean;
}) {
  const [open, setOpen] = useState<number | null>(null);
  const months = useMemo(() => {
    const groups: { label: string; start: number; shots: Shot[] }[] = [];
    const format = new Intl.DateTimeFormat("fr-FR", { month: "long", year: "numeric" });
    shots.forEach((shot) => {
      const label = format.format(new Date(shot.takenAt * 1000));
      const last = groups.at(-1);
      if (last?.label === label) last.shots.push(shot);
      else groups.push({ label, start: groups.reduce((n, g) => n + g.shots.length, 0), shots: [shot] });
    });
    return groups;
  }, [shots]);

  return (
    <div className="space-y-8 px-10">
      {months.map((month) => (
        <section key={month.label}>
          <h2 className="mb-3 font-display text-title font-semibold first-letter:uppercase">{month.label}</h2>
          <ShotGrid
            shots={month.shots}
            titleOf={showTitles ? titleOf : undefined}
            onOpen={(i) => setOpen(month.start + i)}
          />
        </section>
      ))}
      {open !== null && (
        <Lightbox shots={shots} index={open} onIndex={setOpen} onClose={() => setOpen(null)} titleOf={titleOf} />
      )}
    </div>
  );
}

function Chip({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "inline-flex h-8 items-center gap-2 rounded-full border px-3.5 text-caption transition-colors",
        active ? "border-signal bg-signal text-signal-ink" : "border-rivet text-alu-dim hover:border-alu-dim/60 hover:text-alu",
      )}
    >
      {children}
    </button>
  );
}

function Count({ children }: { children: React.ReactNode }) {
  return <span className="tabular-nums opacity-70">{children}</span>;
}
