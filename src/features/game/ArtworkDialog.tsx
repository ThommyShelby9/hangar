import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Check, Search } from "lucide-react";
import { api, type ArtKind, type GameDetail } from "@/lib/api";
import { Modal } from "@/components/Modal";
import { Button } from "@/components/Button";
import { toast } from "@/lib/toast";
import { cn } from "@/lib/utils";

const KINDS: { kind: ArtKind; label: string; shape: string }[] = [
  { kind: "grid", label: "Jaquette", shape: "aspect-[2/3]" },
  { kind: "hero", label: "Fond", shape: "aspect-[96/31]" },
  { kind: "logo", label: "Logo", shape: "aspect-[2/1] object-contain p-3" },
  { kind: "icon", label: "Icône", shape: "aspect-square object-contain p-3" },
];

/** Pick which SteamGridDB game this is, then choose each image. */
export function ArtworkDialog({ game, onClose }: { game: GameDetail; onClose: () => void }) {
  const [searching, setSearching] = useState(game.sgdbId === null);
  const [kind, setKind] = useState<ArtKind>("grid");

  return (
    <Modal open onClose={onClose} title="Images" className="w-[min(60rem,92vw)]">
      {searching || game.sgdbId === null ? (
        <GameSearch game={game} onPicked={() => setSearching(false)} />
      ) : (
        <div className="px-6 py-5">
          <div className="flex flex-wrap items-center gap-3">
            <div role="tablist" className="flex gap-1">
              {KINDS.map((k) => (
                <button
                  key={k.kind}
                  role="tab"
                  aria-selected={kind === k.kind}
                  onClick={() => setKind(k.kind)}
                  className={cn(
                    "h-9 rounded-control px-3.5 transition-colors",
                    kind === k.kind
                      ? "bg-rivet/60 text-alu"
                      : "text-alu-dim hover:bg-rivet/30 hover:text-alu",
                  )}
                >
                  {k.label}
                </button>
              ))}
            </div>
            <Button variant="ghost" className="ml-auto" onClick={() => setSearching(true)}>
              Ce n'est pas le bon jeu ?
            </Button>
          </div>
          <ImageChoices game={game} sgdbId={game.sgdbId} kind={kind} />
        </div>
      )}
    </Modal>
  );
}

function GameSearch({ game, onPicked }: { game: GameDetail; onPicked: () => void }) {
  const [term, setTerm] = useState(game.title);
  const [submitted, setSubmitted] = useState(game.title);
  const results = useQuery({
    queryKey: ["sgdb-search", submitted],
    queryFn: () => api.sgdbSearch(submitted),
    enabled: submitted.trim().length > 0,
  });
  const pick = useMutation({
    mutationFn: (sgdbId: number) => api.setGameSgdb(game.id, sgdbId),
    onSuccess: onPicked,
    onError: toast.error,
  });

  return (
    <div className="px-6 py-5">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          setSubmitted(term);
        }}
        className="flex gap-2"
      >
        <label className="relative flex-1">
          <Search
            size={16}
            className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-alu-dim"
          />
          <input
            value={term}
            onChange={(e) => setTerm(e.target.value)}
            aria-label="Nom du jeu sur SteamGridDB"
            className="h-9 w-full rounded-control border border-rivet bg-bay pr-3 pl-9 focus:border-signal focus:outline-none"
          />
        </label>
        <Button type="submit">Chercher</Button>
      </form>

      {results.error && <p className="mt-4 text-flare">{String(results.error)}</p>}
      {results.data?.length === 0 && (
        <p className="mt-4 text-alu-dim">Aucun jeu trouvé. Essaie un nom plus court.</p>
      )}
      <ul className="mt-4 divide-y divide-rivet">
        {results.data?.map((g) => (
          <li key={g.id}>
            <button
              onClick={() => pick.mutate(g.id)}
              disabled={pick.isPending}
              className="flex w-full items-center gap-3 px-2 py-3 text-left hover:bg-rivet/30"
            >
              <span className="flex-1">{g.name}</span>
              {g.releaseDate && (
                <span className="text-alu-dim tabular-nums">
                  {new Date(g.releaseDate * 1000).getFullYear()}
                </span>
              )}
              {g.id === game.sgdbId && <Check size={16} className="text-primer" />}
            </button>
          </li>
        ))}
      </ul>
      {pick.isPending && <p className="mt-3 text-alu-dim">Téléchargement des images…</p>}
    </div>
  );
}

function ImageChoices({ game, sgdbId, kind }: { game: GameDetail; sgdbId: number; kind: ArtKind }) {
  const images = useQuery({
    queryKey: ["sgdb-images", sgdbId, kind],
    queryFn: () => api.sgdbImages(sgdbId, kind),
  });
  const choose = useMutation({
    mutationFn: (url: string) => api.setGameImage(game.id, kind, url),
    onSuccess: () => toast.info(`${KINDS.find((k) => k.kind === kind)!.label} mise à jour`),
    onError: toast.error,
  });
  const shape = KINDS.find((k) => k.kind === kind)!.shape;

  if (images.error) return <p className="mt-6 text-flare">{String(images.error)}</p>;
  if (images.data?.length === 0) {
    return <p className="mt-6 text-alu-dim">SteamGridDB n'a pas d'image de ce type pour ce jeu.</p>;
  }
  return (
    <ul
      className={cn(
        "mt-5 grid gap-3",
        kind === "grid" ? "grid-cols-5" : kind === "hero" ? "grid-cols-4" : "grid-cols-6",
      )}
    >
      {images.data?.map((img) => (
        <li key={img.id} className={kind === "hero" ? "col-span-2" : undefined}>
          <button
            onClick={() => choose.mutate(img.url)}
            disabled={choose.isPending}
            className="block w-full overflow-hidden rounded-[6px] bg-bay ring-1 ring-white/5 transition hover:ring-2 hover:ring-signal"
          >
            <img
              src={img.thumb}
              alt=""
              loading="lazy"
              className={cn("w-full object-cover", shape)}
            />
          </button>
        </li>
      ))}
    </ul>
  );
}
