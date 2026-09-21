import { useState } from "react";
import { Link } from "react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderSearch, History, Save } from "lucide-react";
import { savesApi, type GameDetail, type GameSaves as Saves } from "@/lib/api";
import { Button } from "@/components/Button";
import { formatBytes, formatDateTime } from "@/lib/format";
import { toast } from "@/lib/toast";

const WHEN = new Intl.DateTimeFormat("fr-FR", { dateStyle: "long", timeStyle: "short" });

/** Where the game's saves are, its backups, and a way to put one back. */
export function GameSaves({ game }: { game: GameDetail }) {
  const queryClient = useQueryClient();
  // Ludusavi takes a second or two: looked up once per visit, not on every focus.
  const { data: saves, isLoading } = useQuery({
    queryKey: ["game-saves", game.id],
    queryFn: () => savesApi.game(game.id),
    staleTime: 60_000,
    refetchOnWindowFocus: false,
  });
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["game-saves", game.id] });

  const backup = useMutation({
    mutationFn: () => savesApi.backup(game.id),
    onSuccess: (done) => {
      toast.info(done ? "Sauvegarde faite" : "Aucun fichier de sauvegarde trouvé pour ce jeu");
      return refresh();
    },
    onError: (e) => {
      toast.error(e);
      return refresh();
    },
  });
  const chooseDir = useMutation({
    mutationFn: async () => {
      const dir = await open({
        directory: true,
        title: `Dossier des sauvegardes de ${game.title}`,
      });
      if (dir) await savesApi.setDir(game.id, dir);
    },
    onSuccess: refresh,
    onError: toast.error,
  });

  return (
    <section className="px-10 pt-10">
      <h2 className="mb-3 font-display text-title font-semibold">Sauvegardes</h2>
      {isLoading ? (
        <p className="text-alu-dim">Recherche des sauvegardes…</p>
      ) : !saves?.installed ? (
        <p className="text-alu-dim">
          Installe Ludusavi dans les{" "}
          <Link to="/reglages" className="text-signal hover:underline">
            Réglages
          </Link>{" "}
          pour sauvegarder ce jeu.
        </p>
      ) : saves.name === null ? (
        <div className="flex flex-wrap items-center gap-4">
          <p className="max-w-[56ch] text-alu-dim">
            Ludusavi ne connaît pas ce jeu. Indique le dossier où il range ses sauvegardes, Hangar
            s'occupera du reste.
          </p>
          <Button onClick={() => chooseDir.mutate()}>
            <FolderSearch size={16} /> Choisir le dossier
          </Button>
        </div>
      ) : (
        <div className="space-y-5">
          <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
            <p>
              {saves.files.length
                ? `${saves.files.length} fichier${saves.files.length > 1 ? "s" : ""} · ${formatBytes(saves.totalBytes)}`
                : "Aucun fichier de sauvegarde trouvé pour l'instant"}
              {saves.lastBackupAt && (
                <span className="text-alu-dim">
                  {" "}
                  · dernière sauvegarde le {formatDateTime(saves.lastBackupAt)}
                </span>
              )}
            </p>
            <div className="flex gap-2">
              <Button
                variant="primary"
                size="sm"
                disabled={backup.isPending || !saves.files.length}
                onClick={() => backup.mutate()}
              >
                <Save size={15} /> {backup.isPending ? "Sauvegarde…" : "Sauvegarder maintenant"}
              </Button>
              <Button size="sm" onClick={() => chooseDir.mutate()}>
                <FolderSearch size={15} />{" "}
                {saves.customDir ? "Changer le dossier" : "Ajouter un dossier"}
              </Button>
            </div>
          </div>
          {saves.lastError && <p className="text-flare">Dernière tentative : {saves.lastError}</p>}
          <SaveLocations saves={saves} />
          <BackupList game={game} saves={saves} onRestored={refresh} />
        </div>
      )}
    </section>
  );
}

/** The folders the save files live in (not every file: some games keep hundreds). */
function SaveLocations({ saves }: { saves: Saves }) {
  const folders = [...new Set(saves.files.map((f) => f.slice(0, f.lastIndexOf("/"))))];
  const extra = [saves.customDir, ...saves.emulatorDirs].filter((d): d is string => !!d);
  if (!folders.length && !extra.length) return null;
  return (
    <details className="text-caption text-alu-dim">
      <summary className="cursor-pointer select-none hover:text-alu">Emplacements</summary>
      <ul className="mt-2 space-y-1 select-text">
        {folders.slice(0, 8).map((f) => (
          <li key={f} className="break-all">
            {f.replaceAll("/", "\\")}
          </li>
        ))}
        {folders.length > 8 && <li>… et {folders.length - 8} autres</li>}
        {extra.map((d) => (
          <li key={d} className="break-all">
            {d} <span className="text-alu-dim/70">(ajouté par Hangar)</span>
          </li>
        ))}
      </ul>
    </details>
  );
}

function BackupList({
  game,
  saves,
  onRestored,
}: {
  game: GameDetail;
  saves: Saves;
  onRestored: () => void;
}) {
  const [confirm, setConfirm] = useState<string | null>(null);
  const restore = useMutation({
    mutationFn: (id: string) => savesApi.restore(game.id, id),
    onSuccess: () => {
      toast.info("Sauvegarde remise en place. L'état d'avant a été sauvegardé, au cas où.");
      setConfirm(null);
      onRestored();
    },
    onError: toast.error,
  });
  if (!saves.backups.length) return null;

  return (
    <ul className="max-w-2xl divide-y divide-rivet border-y border-rivet">
      {saves.backups.map((b, i) => (
        <li key={b.id} className="flex items-center justify-between gap-4 py-2.5">
          <span className="flex items-center gap-2.5">
            <History size={15} className="text-alu-dim" />
            {WHEN.format(new Date(b.when))}
            {i === 0 && <span className="text-caption text-primer">la plus récente</span>}
          </span>
          {confirm === b.id ? (
            <span className="flex items-center gap-2">
              <span className="text-caption text-alu-dim">{game.title} doit être fermé.</span>
              <Button
                size="sm"
                variant="danger"
                disabled={restore.isPending}
                onClick={() => restore.mutate(b.id)}
              >
                {restore.isPending ? "Restauration…" : "Confirmer"}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setConfirm(null)}>
                Annuler
              </Button>
            </span>
          ) : (
            <Button size="sm" variant="ghost" onClick={() => setConfirm(b.id)}>
              Restaurer
            </Button>
          )}
        </li>
      ))}
    </ul>
  );
}
