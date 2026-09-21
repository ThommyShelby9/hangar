import { Link } from "react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Gamepad2 } from "lucide-react";
import { padsApi, type GameDetail, type GamePadInfo, type PadProfile } from "@/lib/api";
import { toast } from "@/lib/toast";

const OUTPUT_LABEL: Record<string, string> = {
  DS4: "manette PlayStation",
  X360: "manette Xbox 360",
};
const describe = (p: PadProfile) => `${p.name} (${OUTPUT_LABEL[p.output] ?? p.output})`;

/** Which DS4Windows profile the game gets when it starts: automatic, or chosen by hand. */
export function GamePad({ game }: { game: GameDetail }) {
  const queryClient = useQueryClient();
  const { data: pad } = useQuery({
    queryKey: ["game-pad", game.id],
    queryFn: () => padsApi.game(game.id),
    staleTime: 60_000,
  });
  const change = useMutation({
    mutationFn: (profile: string | null) => padsApi.set(game.id, profile),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["game-pad", game.id] }),
    onError: toast.error,
  });
  if (!pad?.ds4windows || !pad.profiles.length) return null;

  return (
    <section className="px-10 pt-10">
      <h2 className="mb-3 flex items-center gap-2.5 font-display text-title font-semibold">
        <Gamepad2 size={20} className="text-alu-dim" /> Manette
      </h2>
      {!pad.enabled ? (
        <p className="text-alu-dim">
          Le changement de profil selon le jeu est coupé (page{" "}
          <Link to="/manettes" className="text-signal hover:underline">
            Manettes
          </Link>
          ).
        </p>
      ) : (
        <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
          <select
            aria-label="Profil DS4Windows pour ce jeu"
            value={pad.choice ?? "auto"}
            onChange={(e) => change.mutate(e.target.value === "auto" ? null : e.target.value)}
            className="h-9 rounded-control border border-rivet bg-bay px-3"
          >
            <option value="auto">Automatique</option>
            {pad.profiles.map((p) => (
              <option key={p.name} value={p.name}>
                {describe(p)}
              </option>
            ))}
            <option value="">Ne rien changer</option>
          </select>
          <p className="text-alu-dim">{explain(pad)}</p>
        </div>
      )}
    </section>
  );
}

function explain(pad: GamePadInfo): string {
  const picked = pad.profiles.find((p) => p.name === pad.picked);
  if (pad.choice === "") return "DS4Windows garde ses réglages habituels pendant ce jeu.";
  if (!picked) return "Aucun profil ne convient : DS4Windows garde ses réglages habituels.";
  if (pad.choice !== null) return `Au lancement : ${describe(picked)}.`;
  const why =
    pad.psNative === true
      ? "le jeu reconnaît les manettes PlayStation (boutons ✕ ○ △ □)"
      : pad.psNative === false
        ? "le jeu est fait pour les manettes Xbox"
        : "pas d'information sur ce jeu, la manette Xbox est la plus sûre";
  return `Au lancement : ${describe(picked)}, car ${why}.`;
}
