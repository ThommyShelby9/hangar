import { useState } from "react";
import { Link } from "react-router";
import type { GameDetail } from "@/lib/api";
import { useShots } from "@/features/gallery/queries";
import { ShotGrid } from "@/features/gallery/ShotGrid";
import { Lightbox } from "@/features/gallery/Lightbox";

const PREVIEW = 8;

/** The game's latest captures, on its page. Nothing at all when it has none. */
export function GameShots({ game }: { game: GameDetail }) {
  const { data } = useShots();
  const [open, setOpen] = useState<number | null>(null);
  const shots = (data ?? []).filter((s) => s.gameId === game.id);
  if (!shots.length) return null;

  return (
    <section className="px-10 pt-10">
      <div className="mb-3 flex items-baseline justify-between gap-4">
        <h2 className="font-display text-title font-semibold">
          Captures <span className="text-alu-dim tabular-nums">{shots.length}</span>
        </h2>
        {shots.length > PREVIEW && (
          <Link to={`/captures?jeu=${game.id}`} className="text-caption text-signal hover:underline">
            Tout voir
          </Link>
        )}
      </div>
      <ShotGrid shots={shots.slice(0, PREVIEW)} onOpen={setOpen} />
      {open !== null && (
        <Lightbox
          shots={shots}
          index={open}
          onIndex={setOpen}
          onClose={() => setOpen(null)}
          titleOf={() => game.title}
        />
      )}
    </section>
  );
}
