import { useState, type ReactNode } from "react";
import { useNavigate } from "react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderInput, ImagePlus, Plus, Trash2 } from "lucide-react";
import { api, manageApi, type ArtKind, type GameDetail, type LaunchAction } from "@/lib/api";
import { Modal } from "@/components/Modal";
import { Button } from "@/components/Button";
import { toast } from "@/lib/toast";

const inputClass =
  "h-9 rounded-control border border-rivet bg-bay px-3 placeholder:text-alu-dim/60 focus:border-signal focus:outline-none";

export function EditGameDialog({
  game,
  onClose,
  onChangeArtwork,
}: {
  game: GameDetail;
  onClose: () => void;
  onChangeArtwork: () => void;
}) {
  const [title, setTitle] = useState(game.title);
  // Only local edits are kept: added or removed actions come back through the refreshed game.
  const [edits, setEdits] = useState<Record<number, Partial<LaunchAction>>>({});
  const actions = game.actions.map((a) => ({ ...a, ...edits[a.id] }));

  const updateAction = (id: number, patch: Partial<LaunchAction>) =>
    setEdits((all) => ({ ...all, [id]: { ...all[id], ...patch } }));

  const save = useMutation({
    mutationFn: async () => {
      if (title.trim() && title.trim() !== game.title) {
        await api.updateGame(game.id, { title: title.trim() });
      }
      for (const action of actions) {
        const before = game.actions.find((a) => a.id === action.id);
        if (before && JSON.stringify(before) !== JSON.stringify(action))
          await api.updateAction(action);
      }
    },
    onSuccess: onClose,
    onError: toast.error,
  });

  return (
    <Modal
      open
      onClose={onClose}
      title="Modifier le jeu"
      className="w-[min(50rem,92vw)]"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Annuler
          </Button>
          <Button variant="primary" disabled={save.isPending} onClick={() => save.mutate()}>
            Enregistrer
          </Button>
        </>
      }
    >
      <div className="space-y-9 px-6 py-5">
        <label className="block space-y-1.5">
          <span className="text-alu-dim">Titre</span>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            className={`${inputClass} w-full`}
          />
        </label>

        <Section title="Lancement">
          <ul className="space-y-5">
            {actions.map((action) => (
              <ActionRow
                key={action.id}
                action={action}
                onChange={(p) => updateAction(action.id, p)}
              />
            ))}
          </ul>
          <AddAction gameId={game.id} />
        </Section>

        <Collections game={game} />

        <Section title="Images" hint="Jaquette, fond, logo et icône, depuis SteamGridDB ou ton PC.">
          <div className="flex flex-wrap gap-2">
            <Button onClick={onChangeArtwork}>Choisir sur SteamGridDB</Button>
            <OwnImage gameId={game.id} />
          </div>
        </Section>

        {game.source === "manual" && game.installDir && <Relocate game={game} />}

        <Danger game={game} />
      </div>
    </Modal>
  );
}

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="font-display text-[1.2rem] font-semibold">{title}</h3>
      {hint && <p className="mt-0.5 text-alu-dim">{hint}</p>}
      <div className="mt-3">{children}</div>
    </section>
  );
}

function ActionRow({
  action,
  onChange,
}: {
  action: LaunchAction;
  onChange: (p: Partial<LaunchAction>) => void;
}) {
  const isUri = action.target.includes("://");
  const setDefault = useMutation({
    mutationFn: () => manageApi.setDefaultAction(action.id),
    onError: toast.error,
  });
  const remove = useMutation({
    mutationFn: () => manageApi.removeAction(action.id),
    onError: toast.error,
  });
  return (
    <li className="space-y-2">
      <div className="flex flex-wrap items-center gap-3">
        <p className="font-medium">{action.name}</p>
        {action.isDefault ? (
          <span className="text-caption text-alu-dim">bouton Jouer</span>
        ) : (
          <>
            <Button size="sm" variant="ghost" onClick={() => setDefault.mutate()}>
              Mettre sur le bouton Jouer
            </Button>
            <Button
              size="sm"
              variant="ghost"
              aria-label={`Supprimer ${action.name}`}
              onClick={() => remove.mutate()}
            >
              <Trash2 size={14} />
            </Button>
          </>
        )}
      </div>
      <p className="truncate text-caption text-alu-dim select-text">{action.target}</p>
      {!isUri && (
        <div className="flex flex-wrap items-center gap-x-6 gap-y-2">
          <input
            value={action.args}
            onChange={(e) => onChange({ args: e.target.value })}
            placeholder="Arguments, par ex. -windowed"
            aria-label={`Arguments de ${action.name}`}
            className={`${inputClass} min-w-64 flex-1`}
          />
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={action.runAsAdmin}
              onChange={(e) => onChange({ runAsAdmin: e.target.checked })}
              className="size-4 accent-signal"
            />
            Lancer en administrateur
          </label>
        </div>
      )}
    </li>
  );
}

function AddAction({ gameId }: { gameId: number }) {
  const [name, setName] = useState("");
  const add = useMutation({
    mutationFn: async () => {
      const target = await open({
        title: "Programme à lancer",
        filters: [{ name: "Programmes", extensions: ["exe"] }],
      });
      if (!target) return false;
      await manageApi.addAction(gameId, name.trim() || "Autre", target, "");
      return true;
    },
    onSuccess: (added) => added && setName(""),
    onError: toast.error,
  });
  return (
    <div className="mt-4 flex flex-wrap items-center gap-2">
      <input
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Nom de l'action, par ex. Mods"
        aria-label="Nom de la nouvelle action"
        className={`${inputClass} w-64`}
      />
      <Button onClick={() => add.mutate()} disabled={add.isPending}>
        <Plus size={16} />
        Ajouter une action
      </Button>
    </div>
  );
}

function Collections({ game }: { game: GameDetail }) {
  const queryClient = useQueryClient();
  const { data: collections = [] } = useQuery({
    queryKey: ["collections"],
    queryFn: manageApi.collections,
  });
  const [name, setName] = useState("");
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["collections"] });

  const toggle = useMutation({
    mutationFn: (id: number) =>
      manageApi.setGameCollections(
        game.id,
        game.collections.includes(id)
          ? game.collections.filter((c) => c !== id)
          : [...game.collections, id],
      ),
    onSuccess: refresh,
    onError: toast.error,
  });
  const create = useMutation({
    mutationFn: async () => {
      const id = await manageApi.createCollection(name);
      await manageApi.setGameCollections(game.id, [...game.collections, id]);
    },
    onSuccess: () => {
      setName("");
      refresh();
    },
    onError: toast.error,
  });

  return (
    <Section title="Collections">
      {collections.length > 0 && (
        <div className="mb-3 flex flex-wrap gap-2">
          {collections.map((c) => {
            const checked = game.collections.includes(c.id);
            return (
              <button
                key={c.id}
                role="checkbox"
                aria-checked={checked}
                onClick={() => toggle.mutate(c.id)}
                className={
                  checked
                    ? "h-8 rounded-full bg-signal px-3.5 text-signal-ink"
                    : "h-8 rounded-full border border-rivet px-3.5 text-alu-dim hover:text-alu"
                }
              >
                {c.name}
              </button>
            );
          })}
        </div>
      )}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (name.trim()) create.mutate();
        }}
        className="flex flex-wrap gap-2"
      >
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Nouvelle collection, par ex. À finir"
          aria-label="Nom de la nouvelle collection"
          className={`${inputClass} w-64`}
        />
        <Button type="submit" disabled={!name.trim()}>
          Créer et ajouter
        </Button>
      </form>
    </Section>
  );
}

const KIND_LABELS: Record<ArtKind, string> = {
  grid: "Jaquette",
  hero: "Fond",
  logo: "Logo",
  icon: "Icône",
};

function OwnImage({ gameId }: { gameId: number }) {
  const [kind, setKind] = useState<ArtKind>("grid");
  const pick = useMutation({
    mutationFn: async () => {
      const path = await open({
        title: "Choisir une image",
        filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "ico"] }],
      });
      if (path) await manageApi.setImageFile(gameId, kind, path);
      return Boolean(path);
    },
    onSuccess: (done) => done && toast.info(`${KIND_LABELS[kind]} mise à jour`),
    onError: toast.error,
  });
  return (
    <div className="flex gap-2">
      <select
        value={kind}
        onChange={(e) => setKind(e.target.value as ArtKind)}
        aria-label="Type d'image"
        className={inputClass}
      >
        {Object.entries(KIND_LABELS).map(([value, label]) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
      </select>
      <Button onClick={() => pick.mutate()} disabled={pick.isPending}>
        <ImagePlus size={16} />
        Depuis mon PC
      </Button>
    </div>
  );
}

export function Relocate({ game }: { game: GameDetail }) {
  const move = useMutation({
    mutationFn: async () => {
      const dir = await open({ directory: true, title: `Nouveau dossier de ${game.title}` });
      if (dir) await manageApi.relocate(game.id, dir);
      return Boolean(dir);
    },
    onSuccess: (done) => done && toast.info(`${game.title} a été retrouvé.`),
    onError: toast.error,
  });
  return (
    <Section
      title="Dossier du jeu"
      hint="Si tu as déplacé le jeu ou si la lettre du disque a changé, indique son nouveau dossier : le temps de jeu et les images sont gardés."
    >
      <p className="mb-2 text-caption text-alu-dim select-text">{game.installDir}</p>
      <Button onClick={() => move.mutate()} disabled={move.isPending}>
        <FolderInput size={16} />
        Indiquer le nouveau dossier
      </Button>
    </Section>
  );
}

function Danger({ game }: { game: GameDetail }) {
  const navigate = useNavigate();
  const [confirm, setConfirm] = useState<"remove" | "recycle" | null>(null);
  const { data: info } = useQuery({
    queryKey: ["uninstall", game.id],
    queryFn: () => manageApi.uninstallInfo(game.id),
    enabled: game.source === "manual",
  });

  const remove = useMutation({
    mutationFn: () => api.removeGame(game.id),
    onSuccess: () => {
      toast.info(`${game.title} a été retiré de la bibliothèque.`);
      navigate("/bibliotheque");
    },
    onError: toast.error,
  });
  const uninstall = useMutation({
    mutationFn: (mode: "uninstaller" | "recycle") => manageApi.uninstall(game.id, mode),
    onSuccess: (_, mode) => {
      if (mode === "recycle") {
        toast.info(`${game.title} est dans la corbeille.`);
        navigate("/bibliotheque");
      } else {
        toast.info("Le désinstalleur du jeu est ouvert.");
      }
    },
    onError: toast.error,
  });

  return (
    <section className="space-y-4 border-t border-rivet pt-6">
      {confirm === "remove" ? (
        <Confirm
          text={`Retirer ${game.title} de Hangar ? Les fichiers du jeu restent sur le disque.`}
          action="Retirer"
          onCancel={() => setConfirm(null)}
          onConfirm={() => remove.mutate()}
        />
      ) : confirm === "recycle" ? (
        <Confirm
          text={`Envoyer tout le dossier ${info?.installDir} à la corbeille ? Tu pourras le restaurer depuis la corbeille tant qu'elle n'est pas vidée.`}
          action="Mettre à la corbeille"
          onCancel={() => setConfirm(null)}
          onConfirm={() => uninstall.mutate("recycle")}
        />
      ) : (
        <div className="flex flex-wrap gap-2">
          <Button variant="danger" onClick={() => setConfirm("remove")}>
            Retirer de la bibliothèque
          </Button>
          {info?.uninstaller && (
            <Button variant="danger" onClick={() => uninstall.mutate("uninstaller")}>
              Désinstaller le jeu
            </Button>
          )}
          {info?.installDir && !info.uninstaller && (
            <Button variant="danger" onClick={() => setConfirm("recycle")}>
              Supprimer le dossier du jeu
            </Button>
          )}
        </div>
      )}
    </section>
  );
}

function Confirm({
  text,
  action,
  onCancel,
  onConfirm,
}: {
  text: string;
  action: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-3">
      <p className="mr-auto max-w-[52ch]">{text}</p>
      <Button variant="ghost" onClick={onCancel}>
        Annuler
      </Button>
      <Button variant="danger" onClick={onConfirm}>
        {action}
      </Button>
    </div>
  );
}
