import { useState, type CSSProperties } from "react";
import { Link, useParams } from "react-router";
import { useMutation } from "@tanstack/react-query";
import { ArrowLeft, Heart, Pencil, Play, TriangleAlert } from "lucide-react";
import { api, imageUrl, type GameDetail } from "@/lib/api";
import { Button } from "@/components/Button";
import { toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { useGame, useGameStats, useRunningGames } from "@/features/library/queries";
import { GameCover } from "@/features/library/GameCover";
import { useDominantColor } from "@/lib/color";
import { motion } from "motion/react";
import { formatDateTime, formatDuration, formatShortDate } from "@/lib/format";
import { EditGameDialog } from "@/features/game/EditGameDialog";
import { ArtworkDialog } from "@/features/game/ArtworkDialog";
import { GameShots } from "@/features/gallery/GameShots";
import { GameSaves } from "@/features/saves/GameSaves";

const SOURCE_LABEL: Record<GameDetail["source"], string> = {
  manual: "Ajouté à la main",
  steam: "Steam",
  epic: "Epic Games",
};

export function GamePage() {
  const id = Number(useParams().id);
  const { data: game, error } = useGame(id);

  if (error) {
    return (
      <div className="px-10 pt-10">
        <BackLink />
        <p className="mt-6 text-flare">{String(error)}</p>
      </div>
    );
  }
  if (!game) return null;
  return <GameView game={game} />;
}

function GameView({ game }: { game: GameDetail }) {
  const [dialog, setDialog] = useState<"edit" | "artwork" | null>(null);
  const hero = imageUrl(game.images.hero);
  const cover = imageUrl(game.images.grid);
  const logo = imageUrl(game.images.logo);
  const missing = game.status === "missing";
  const running = useRunningGames().has(game.id);
  const { data: stats } = useGameStats(game.id);
  const [primary, ...others] = game.actions;
  const accent = useDominantColor(cover);

  const launch = useMutation({
    mutationFn: (actionId?: number) => api.launchGame(game.id, actionId),
    onSuccess: () => toast.info(`Lancement de ${game.title}…`),
    onError: toast.error,
  });
  const favorite = useMutation({
    mutationFn: () => api.updateGame(game.id, { favorite: !game.favorite }),
    onError: toast.error,
  });

  return (
    <article
      className="min-h-full"
      style={accent ? ({ "--accent": accent } as CSSProperties) : undefined}
    >
      <section className="relative isolate flex h-[52vh] min-h-[26rem] flex-col justify-between overflow-hidden px-10 pt-6 pb-8">
        {hero ? (
          <img src={hero} alt="" className="absolute inset-0 -z-10 size-full object-cover" />
        ) : cover ? (
          <img
            src={cover}
            alt=""
            className="absolute inset-0 -z-10 size-full scale-110 object-cover blur-2xl saturate-150"
          />
        ) : null}
        {/* Fades the artwork into the page and keeps the title legible. */}
        <div className="absolute inset-0 -z-10 bg-gradient-to-t from-tarmac via-tarmac/55 to-tarmac/10" />
        <div className="absolute inset-0 -z-10 bg-gradient-to-r from-tarmac/70 to-transparent" />
        {/* The cover's own colour glows under the title. */}
        <div
          aria-hidden
          className="absolute inset-0 -z-10 transition-opacity duration-700"
          style={{
            opacity: accent ? 1 : 0,
            background:
              "radial-gradient(55% 75% at 18% 92%, rgb(var(--accent) / 0.42), transparent 72%)",
          }}
        />

        <BackLink />

        <div className="flex items-end gap-8">
          {/* Same layoutId as the library card: the cover flies from the grid to here. */}
          <motion.div
            layoutId={`cover-${game.id}`}
            transition={{ type: "spring", stiffness: 260, damping: 30 }}
            className="hidden w-40 shrink-0 shadow-2xl shadow-black/60 lg:block"
          >
            <GameCover game={game} />
          </motion.div>
          <div className="min-w-0">
            {logo ? (
              <img
                src={logo}
                alt={game.title}
                className="max-h-36 max-w-[min(30rem,70%)] object-contain object-left drop-shadow-[0_4px_24px_rgb(0_0_0/0.6)]"
              />
            ) : (
              <h1 className="max-w-[20ch] font-display text-[4rem] leading-[0.9] font-bold drop-shadow-lg">
                {game.title}
              </h1>
            )}

            <div className="mt-7 flex flex-wrap items-center gap-3">
              <Button
                variant="primary"
                size="lg"
                disabled={!primary || missing || running || launch.isPending}
                onClick={() => launch.mutate(primary?.id)}
                className="min-w-44"
              >
                {running ? (
                  <>
                    <span className="size-2.5 rounded-full bg-current" />
                    En cours
                  </>
                ) : (
                  <>
                    <Play size={20} className="fill-current" />
                    Jouer
                  </>
                )}
              </Button>
              {others.map((action) => (
                <Button
                  key={action.id}
                  disabled={missing}
                  onClick={() => launch.mutate(action.id)}
                  className="h-12 bg-bay/80 backdrop-blur"
                >
                  {action.name}
                </Button>
              ))}
              <Button
                variant="ghost"
                size="icon"
                aria-label={game.favorite ? "Retirer des favoris" : "Ajouter aux favoris"}
                aria-pressed={game.favorite}
                onClick={() => favorite.mutate()}
                className="size-12"
              >
                <Heart size={20} className={cn(game.favorite && "fill-signal text-signal")} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Modifier le jeu"
                onClick={() => setDialog("edit")}
                className="size-12"
              >
                <Pencil size={18} />
              </Button>
            </div>
          </div>
        </div>
      </section>

      {missing && (
        <p className="mx-10 mt-2 flex items-center gap-2 rounded-control border border-flare/40 bg-flare/10 px-4 py-3 text-flare">
          <TriangleAlert size={16} />
          <span className="flex-1">
            Hangar ne trouve plus ce jeu. Le disque est peut-être débranché ou le dossier a été
            déplacé.
          </span>
          {game.source === "manual" && game.installDir && (
            <Button size="sm" variant="danger" onClick={() => setDialog("edit")}>
              Retrouver le jeu
            </Button>
          )}
        </p>
      )}

      <section className="flex flex-wrap items-end gap-x-14 gap-y-4 px-10 pt-8">
        <Figure label="Temps de jeu" value={stats ? formatDuration(stats.totalSecs) : "…"} />
        <Figure label="Sessions" value={stats ? String(stats.sessionCount) : "…"} />
        <Figure
          label="Dernière session"
          value={stats?.lastSession ? formatDuration(stats.lastSession.durationSecs) : "—"}
          detail={stats?.lastSession && formatShortDate(stats.lastSession.startedAt)}
        />
      </section>

      <GameShots game={game} />
      <GameSaves game={game} />

      <dl className="grid max-w-3xl grid-cols-[10rem_1fr] gap-x-6 gap-y-3 px-10 py-8">
        <dt className="text-alu-dim">Provenance</dt>
        <dd>{SOURCE_LABEL[game.source]}</dd>
        {game.installDir && (
          <>
            <dt className="text-alu-dim">Dossier</dt>
            <dd className="break-all select-text">{game.installDir}</dd>
          </>
        )}
        <dt className="text-alu-dim">Dernière partie</dt>
        <dd>{game.lastPlayedAt ? formatDateTime(game.lastPlayedAt) : "Jamais lancé"}</dd>
        <dt className="text-alu-dim">Ajouté le</dt>
        <dd>{formatDateTime(game.addedAt)}</dd>
      </dl>

      {dialog === "edit" && (
        <EditGameDialog
          game={game}
          onClose={() => setDialog(null)}
          onChangeArtwork={() => setDialog("artwork")}
        />
      )}
      {dialog === "artwork" && <ArtworkDialog game={game} onClose={() => setDialog(null)} />}
    </article>
  );
}

function BackLink() {
  return (
    <Link
      to="/bibliotheque"
      className="inline-flex w-fit items-center gap-2 rounded-control py-1 pr-2 text-alu/80 hover:text-alu"
    >
      <ArrowLeft size={16} />
      Bibliothèque
    </Link>
  );
}

function Figure({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail?: string | null;
}) {
  return (
    <div>
      <p className="text-alu-dim">{label}</p>
      <p className="mt-1 text-[1.75rem] leading-none font-semibold">{value}</p>
      {detail && <p className="mt-1.5 text-caption text-alu-dim">{detail}</p>}
    </div>
  );
}
