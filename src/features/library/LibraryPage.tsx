import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, motion } from "motion/react";
import Fuse from "fuse.js";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { FolderSearch, Plus, Search } from "lucide-react";
import { api, manageApi, type FolderCandidate, type Game } from "@/lib/api";
import { PageHeader } from "@/components/PageHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/Button";
import { toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import {
  newGamesKey,
  useGames,
  useNewGamesCount,
  useRunningGames,
} from "@/features/library/queries";
import { GameCard } from "@/features/library/GameCard";
import { ImportDialog } from "@/features/library/ImportDialog";

type Filter = "all" | "favorites" | "missing" | `collection-${number}`;

function matchesFilter(game: Game, filter: Filter) {
  if (filter === "all") return true;
  if (filter === "favorites") return game.favorite;
  if (filter === "missing") return game.status === "missing";
  return game.collections.includes(Number(filter.slice("collection-".length)));
}

const SORTS = {
  title: { label: "Nom", compare: (a: Game, b: Game) => a.sortTitle.localeCompare(b.sortTitle) },
  recent: {
    label: "Dernière partie",
    compare: (a: Game, b: Game) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0),
  },
  playtime: {
    label: "Temps de jeu",
    compare: (a: Game, b: Game) => b.playtimeSecs - a.playtimeSecs,
  },
  added: { label: "Date d'ajout", compare: (a: Game, b: Game) => b.addedAt - a.addedAt },
} as const;

type Sort = keyof typeof SORTS;

export function LibraryPage() {
  const { data: games = [], isPending } = useGames();
  const { data: collections = [] } = useQuery({
    queryKey: ["collections"],
    queryFn: manageApi.collections,
  });
  const queryClient = useQueryClient();
  const newGames = useNewGamesCount();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [sort, setSort] = useState<Sort>("title");
  const running = useRunningGames();
  const [candidates, setCandidates] = useState<FolderCandidate[] | null>(null);
  const dragging = useDroppedGames(setCandidates);

  const scan = useMutation({
    mutationFn: api.scanFolders,
    onSuccess: (found) => {
      queryClient.setQueryData(newGamesKey, 0);
      if (found.length === 0) toast.info("Aucun nouveau jeu dans les dossiers surveillés.");
      else setCandidates(found);
    },
    onError: toast.error,
  });

  const addFiles = useMutation({
    mutationFn: async () => {
      const picked = await open({
        multiple: true,
        title: "Choisir le jeu ou son raccourci",
        filters: [{ name: "Jeux et raccourcis", extensions: ["exe", "lnk", "url"] }],
      });
      return picked?.length ? api.analyzePaths(picked) : null;
    },
    onSuccess: (found) => found && setCandidates(found),
    onError: toast.error,
  });

  const missingCount = games.filter((g) => g.status === "missing").length;
  const filters: [Filter, string][] = [
    ["all", "Tous"],
    ["favorites", "Favoris"],
    ...(missingCount > 0 ? ([["missing", "Introuvables"]] as [Filter, string][]) : []),
    ...collections.map((c): [Filter, string] => [`collection-${c.id}`, c.name]),
  ];

  const visible = useMemo(() => {
    const shown = games.filter((g) => !g.hidden && matchesFilter(g, filter));
    if (!query.trim()) return [...shown].sort(SORTS[sort].compare);
    return new Fuse(shown, { keys: ["title"], threshold: 0.35, ignoreLocation: true })
      .search(query.trim())
      .map((r) => r.item);
  }, [games, query, filter, sort]);

  const recent = useMemo(
    () =>
      games
        .filter((g) => !g.hidden && g.lastPlayedAt)
        .sort((a, b) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0))
        .slice(0, 6),
    [games],
  );
  const showRecent = filter === "all" && !query.trim() && recent.length > 0;

  const actions = (
    <>
      <Button onClick={() => scan.mutate()} disabled={scan.isPending}>
        <FolderSearch size={16} />
        {scan.isPending ? "Recherche…" : "Chercher des jeux"}
      </Button>
      <Button onClick={() => addFiles.mutate()}>
        <Plus size={16} />
        Ajouter un jeu
      </Button>
    </>
  );

  const libraryEmpty = !isPending && games.length === 0;

  return (
    <div className="relative min-h-full">
      <PageHeader title="Bibliothèque" actions={libraryEmpty ? undefined : actions} />

      {newGames > 0 && !libraryEmpty && (
        <div className="mx-10 mb-6 flex max-w-3xl flex-wrap items-center gap-4 rounded-panel border border-signal/40 bg-signal/10 px-5 py-3">
          <FolderSearch size={18} className="text-signal" />
          <p className="flex-1">
            {newGames === 1
              ? "1 nouveau jeu dans tes dossiers."
              : `${newGames} nouveaux jeux dans tes dossiers.`}
          </p>
          <Button
            variant="primary"
            size="sm"
            onClick={() => scan.mutate()}
            disabled={scan.isPending}
          >
            Voir
          </Button>
        </div>
      )}

      {libraryEmpty ? (
        <EmptyState title="Le hangar est vide" art={<EmptyStand />}>
          Hangar cherche tes jeux dans les dossiers surveillés (H:\Games et N:\ pour l'instant). Tu
          peux aussi glisser un .exe dans la fenêtre.
          <span className="mt-6 flex justify-center gap-2">{actions}</span>
        </EmptyState>
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-3 px-10 pb-6">
            <label className="relative w-72">
              <Search
                size={16}
                className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-alu-dim"
              />
              <input
                type="search"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Rechercher un jeu"
                aria-label="Rechercher un jeu"
                className="h-9 w-full rounded-control border border-rivet bg-bay pr-3 pl-9 placeholder:text-alu-dim/70 focus:border-signal focus:outline-none"
              />
            </label>
            <div role="radiogroup" aria-label="Filtre" className="flex flex-wrap gap-1">
              {filters.map(([value, label]) => (
                <button
                  key={value}
                  role="radio"
                  aria-checked={filter === value}
                  onClick={() => setFilter(value)}
                  className={cn(
                    "h-9 rounded-control px-3.5 transition-colors",
                    filter === value
                      ? "bg-rivet/60 text-alu"
                      : "text-alu-dim hover:bg-rivet/30 hover:text-alu",
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
            <label className="flex items-center gap-2 text-alu-dim">
              Trier par
              <select
                value={sort}
                onChange={(e) => setSort(e.target.value as Sort)}
                className="h-9 rounded-control border border-rivet bg-bay px-2 text-alu focus:border-signal focus:outline-none"
              >
                {Object.entries(SORTS).map(([value, { label }]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
            <p className="ml-auto text-alu-dim tabular-nums">
              {visible.length} {visible.length > 1 ? "jeux" : "jeu"}
            </p>
          </div>

          {visible.length === 0 ? (
            <EmptyState title="Aucun jeu ne correspond">
              {query
                ? `Rien pour « ${query} ».`
                : filter === "favorites"
                  ? "Tu n'as pas encore de favori."
                  : "Aucun jeu dans cette collection."}
            </EmptyState>
          ) : (
            <>
              {showRecent && (
                <section className="px-10 pb-10">
                  <h2 className="mb-4 font-display text-title font-semibold">Joués récemment</h2>
                  <ul className="grid grid-cols-[repeat(auto-fill,minmax(10.5rem,1fr))] gap-x-5">
                    {recent.map((game) => (
                      <li key={game.id}>
                        <GameCard game={game} running={running.has(game.id)} shared={false} />
                      </li>
                    ))}
                  </ul>
                  <h2 className="mt-10 font-display text-title font-semibold">Tous les jeux</h2>
                </section>
              )}
              <ul className="grid grid-cols-[repeat(auto-fill,minmax(10.5rem,1fr))] gap-x-5 gap-y-7 px-10 pb-12">
                {visible.map((game) => (
                  <li key={game.id}>
                    <GameCard game={game} running={running.has(game.id)} />
                  </li>
                ))}
              </ul>
            </>
          )}
        </>
      )}

      <AnimatePresence>
        {dragging && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="pointer-events-none fixed inset-0 z-40 grid place-items-center bg-tarmac/80 backdrop-blur-sm"
          >
            <div className="rounded-panel border-2 border-dashed border-signal px-16 py-12 text-center">
              <p className="font-display text-title font-semibold">Dépose ici</p>
              <p className="mt-1 text-alu-dim">Un .exe, un raccourci ou un dossier de jeu</p>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {candidates && <ImportDialog candidates={candidates} onClose={() => setCandidates(null)} />}
    </div>
  );
}

/** Files dropped on the window go through the same review as a folder scan. */
function useDroppedGames(onFound: (candidates: FolderCandidate[]) => void) {
  const [dragging, setDragging] = useState(false);
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(async (event) => {
      const { type } = event.payload;
      if (type === "enter" || type === "over") setDragging(true);
      else if (type === "leave") setDragging(false);
      else if (type === "drop") {
        setDragging(false);
        try {
          const found = await api.analyzePaths(event.payload.paths);
          if (found.length === 0) toast.error("Aucun jeu reconnu dans ce que tu as déposé.");
          else onFound(found);
        } catch (e) {
          toast.error(e);
        }
      }
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, [onFound]);
  return dragging;
}

/** An empty aircraft stand seen from above: painted lead-in line, stop bar, stand outline. */
function EmptyStand() {
  const draw = { hidden: { pathLength: 0 }, shown: { pathLength: 1 } };
  return (
    <svg viewBox="0 0 240 150" className="mb-8 w-60" aria-hidden="true" fill="none">
      <rect
        x="56"
        y="12"
        width="128"
        height="108"
        rx="2"
        className="stroke-alu-dim/35"
        strokeWidth="1.5"
        strokeDasharray="7 6"
      />
      <motion.path
        d="M 4 146 C 70 146 120 138 120 96 L 120 40"
        className="stroke-signal"
        strokeWidth="3"
        strokeLinecap="round"
        variants={draw}
        initial="hidden"
        animate="shown"
        transition={{ duration: 1.1, ease: [0.3, 0, 0.2, 1] }}
      />
      <motion.line
        x1="100"
        y1="32"
        x2="140"
        y2="32"
        className="stroke-signal"
        strokeWidth="5"
        strokeLinecap="round"
        variants={draw}
        initial="hidden"
        animate="shown"
        transition={{ delay: 1, duration: 0.3 }}
      />
    </svg>
  );
}
