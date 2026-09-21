import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Star } from "lucide-react";
import { api, type GameDetail, type GamePatch, type Progress } from "@/lib/api";
import { toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { PROGRESS_LABEL } from "@/features/game/progress";

/** Where the player is with the game, a rating, and personal notes (codes, where they stopped…). */
export function GameProgress({ game }: { game: GameDetail }) {
  const update = useMutation({
    mutationFn: (patch: GamePatch) => api.updateGame(game.id, patch),
    onError: toast.error,
  });
  const [notes, setNotes] = useState(game.notes ?? "");
  const [hover, setHover] = useState<number | null>(null);
  const stars = hover ?? game.rating ?? 0;

  return (
    <section className="grid max-w-5xl gap-x-12 gap-y-6 px-10 pt-10 lg:grid-cols-[auto_1fr]">
      <div className="space-y-4">
        <h2 className="font-display text-title font-semibold">Progression</h2>
        <div role="radiogroup" aria-label="Progression" className="flex flex-wrap gap-2">
          {(Object.keys(PROGRESS_LABEL) as Progress[]).map((p) => (
            <button
              key={p}
              role="radio"
              aria-checked={game.progress === p}
              // A second click on the current status clears it.
              onClick={() => update.mutate({ progress: game.progress === p ? "" : p })}
              className={cn(
                "h-8 rounded-full border px-3.5 text-caption transition-colors",
                game.progress === p
                  ? "border-signal bg-signal text-signal-ink"
                  : "border-rivet text-alu-dim hover:border-alu-dim/60 hover:text-alu",
              )}
            >
              {PROGRESS_LABEL[p]}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-1" onMouseLeave={() => setHover(null)}>
          {[1, 2, 3, 4, 5].map((n) => (
            <button
              key={n}
              aria-label={`${n} étoile${n > 1 ? "s" : ""}`}
              onMouseEnter={() => setHover(n)}
              onClick={() => update.mutate({ rating: game.rating === n ? 0 : n })}
              className="rounded p-0.5"
            >
              <Star
                size={22}
                className={cn(
                  "transition-colors",
                  n <= stars ? "fill-signal text-signal" : "text-rivet",
                )}
              />
            </button>
          ))}
        </div>
      </div>
      <label className="flex flex-col gap-2">
        <span className="text-alu-dim">Notes perso</span>
        <textarea
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          onBlur={() => notes !== (game.notes ?? "") && update.mutate({ notes })}
          rows={4}
          placeholder="Où tu t'es arrêté, un code, une astuce, les trophées qui restent…"
          className="resize-y rounded-control border border-rivet bg-bay px-3 py-2 placeholder:text-alu-dim/60"
        />
      </label>
    </section>
  );
}
