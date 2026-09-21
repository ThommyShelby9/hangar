import { useState } from "react";
import { useNavigate } from "react-router";
import { useMutation } from "@tanstack/react-query";
import { api, type GameDetail, type LaunchAction } from "@/lib/api";
import { Modal } from "@/components/Modal";
import { Button } from "@/components/Button";
import { toast } from "@/lib/toast";

export function EditGameDialog({
  game,
  onClose,
  onChangeArtwork,
}: {
  game: GameDetail;
  onClose: () => void;
  onChangeArtwork: () => void;
}) {
  const navigate = useNavigate();
  const [title, setTitle] = useState(game.title);
  const [actions, setActions] = useState<LaunchAction[]>(game.actions);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const updateAction = (id: number, patch: Partial<LaunchAction>) =>
    setActions((list) => list.map((a) => (a.id === id ? { ...a, ...patch } : a)));

  const save = useMutation({
    mutationFn: async () => {
      if (title.trim() && title.trim() !== game.title) {
        await api.updateGame(game.id, { title: title.trim() });
      }
      for (const action of actions) {
        const before = game.actions.find((a) => a.id === action.id);
        if (JSON.stringify(before) !== JSON.stringify(action)) await api.updateAction(action);
      }
    },
    onSuccess: onClose,
    onError: toast.error,
  });

  const remove = useMutation({
    mutationFn: () => api.removeGame(game.id),
    onSuccess: () => {
      toast.info(`${game.title} a été retiré de la bibliothèque.`);
      navigate("/bibliotheque");
    },
    onError: toast.error,
  });

  return (
    <Modal
      open
      onClose={onClose}
      title="Modifier le jeu"
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
      <div className="space-y-8 px-6 py-5">
        <Field label="Titre">
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            className="h-9 w-full rounded-control border border-rivet bg-bay px-3 focus:border-signal focus:outline-none"
          />
        </Field>

        <section>
          <h3 className="font-display text-[1.2rem] font-semibold">Lancement</h3>
          <ul className="mt-3 space-y-5">
            {actions.map((action) => (
              <li key={action.id} className="space-y-2">
                <p className="font-medium">
                  {action.name}
                  {action.isDefault && <span className="text-alu-dim"> · bouton Jouer</span>}
                </p>
                <p className="truncate text-caption text-alu-dim select-text">{action.target}</p>
                {!action.target.includes("://") && (
                  <div className="flex flex-wrap items-center gap-x-6 gap-y-2">
                    <input
                      value={action.args}
                      onChange={(e) => updateAction(action.id, { args: e.target.value })}
                      placeholder="Arguments, par ex. -windowed"
                      aria-label={`Arguments de ${action.name}`}
                      className="h-9 min-w-64 flex-1 rounded-control border border-rivet bg-bay px-3 placeholder:text-alu-dim/60 focus:border-signal focus:outline-none"
                    />
                    <label className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={action.runAsAdmin}
                        onChange={(e) => updateAction(action.id, { runAsAdmin: e.target.checked })}
                        className="size-4 accent-signal"
                      />
                      Lancer en administrateur
                    </label>
                  </div>
                )}
              </li>
            ))}
          </ul>
        </section>

        <section>
          <h3 className="font-display text-[1.2rem] font-semibold">Images</h3>
          <p className="mt-1 text-alu-dim">
            Jaquette, fond, logo et icône viennent de SteamGridDB.
          </p>
          <Button className="mt-3" onClick={onChangeArtwork}>
            Changer les images
          </Button>
        </section>

        <section className="border-t border-rivet pt-6">
          {confirmRemove ? (
            <div className="flex flex-wrap items-center gap-3">
              <p className="mr-auto">
                Retirer {game.title} de Hangar ? Les fichiers du jeu restent sur le disque.
              </p>
              <Button variant="ghost" onClick={() => setConfirmRemove(false)}>
                Garder
              </Button>
              <Button variant="danger" disabled={remove.isPending} onClick={() => remove.mutate()}>
                Retirer
              </Button>
            </div>
          ) : (
            <Button variant="danger" onClick={() => setConfirmRemove(true)}>
              Retirer de la bibliothèque
            </Button>
          )}
        </section>
      </div>
    </Modal>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1.5">
      <span className="text-alu-dim">{label}</span>
      {children}
    </label>
  );
}
