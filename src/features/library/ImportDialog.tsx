import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { api, type FolderCandidate } from "@/lib/api";
import { Modal } from "@/components/Modal";
import { Button } from "@/components/Button";
import { toast } from "@/lib/toast";

interface Row {
  candidate: FolderCandidate;
  selected: boolean;
  title: string;
  exe: string;
}

/** Review of the games found in folders or dropped in the window, before they join the library. */
export function ImportDialog({
  candidates,
  onClose,
}: {
  candidates: FolderCandidate[];
  onClose: () => void;
}) {
  const [rows, setRows] = useState<Row[]>(() =>
    candidates
      .filter((c) => c.kind === "game")
      .map((c) => ({ candidate: c, selected: true, title: c.title, exe: c.exes[0].path })),
  );
  const installers = candidates.filter((c) => c.kind === "installer");
  const selectedCount = rows.filter((r) => r.selected).length;

  const update = (i: number, patch: Partial<Row>) =>
    setRows((rs) => rs.map((r, j) => (j === i ? { ...r, ...patch } : r)));

  const confirm = useMutation({
    mutationFn: async () => {
      const chosen = rows.filter((r) => r.selected && r.title.trim());
      await api.importGames(
        chosen.map((r) => ({
          dir: r.candidate.dir,
          title: r.title.trim(),
          exe: r.exe,
          extras: r.candidate.extras,
        })),
      );
      await api.ignoreFolders(rows.filter((r) => !r.selected).map((r) => r.candidate.dir));
      return chosen.length;
    },
    onSuccess: (count) => {
      toast.info(count === 1 ? "1 jeu ajouté" : `${count} jeux ajoutés`);
      onClose();
    },
    onError: toast.error,
  });

  return (
    <Modal
      open
      onClose={onClose}
      title={rows.length === 1 ? "1 jeu trouvé" : `${rows.length} jeux trouvés`}
      footer={
        <>
          <p className="mr-auto text-caption text-alu-dim">
            Les jeux décochés ne seront plus proposés.
          </p>
          <Button variant="ghost" onClick={onClose}>
            Annuler
          </Button>
          <Button
            variant="primary"
            disabled={confirm.isPending || rows.length === 0}
            onClick={() => confirm.mutate()}
          >
            {selectedCount === 1 ? "Ajouter 1 jeu" : `Ajouter ${selectedCount} jeux`}
          </Button>
        </>
      }
    >
      <ul className="divide-y divide-rivet">
        {rows.map((row, i) => (
          <li key={row.candidate.dir} className="flex gap-4 px-6 py-4">
            <input
              type="checkbox"
              checked={row.selected}
              onChange={(e) => update(i, { selected: e.target.checked })}
              aria-label={`Ajouter ${row.title}`}
              className="mt-2.5 size-4 accent-signal"
            />
            <div className="min-w-0 flex-1 space-y-2">
              <input
                value={row.title}
                onChange={(e) => update(i, { title: e.target.value })}
                aria-label="Titre"
                className="-ml-2 w-full rounded-control border border-transparent bg-transparent px-2 py-1 font-display text-[1.25rem] font-semibold hover:border-rivet focus:border-signal focus:outline-none"
              />
              {row.candidate.exes.length > 1 ? (
                <select
                  value={row.exe}
                  onChange={(e) => update(i, { exe: e.target.value })}
                  aria-label="Exécutable du jeu"
                  className="w-full rounded-control border border-rivet bg-bay px-2 py-1.5 text-caption"
                >
                  {row.candidate.exes.map((exe) => (
                    <option key={exe.path} value={exe.path}>
                      {relative(exe.path, row.candidate.dir)}
                      {exe.productName ? ` — ${exe.productName}` : ""}
                    </option>
                  ))}
                </select>
              ) : (
                <p className="truncate text-caption text-alu-dim select-text">{row.exe}</p>
              )}
              {row.candidate.extras.length > 0 && (
                <p className="text-caption text-alu-dim">
                  Actions en plus : {row.candidate.extras.map((e) => e.name).join(", ")}
                </p>
              )}
            </div>
          </li>
        ))}
      </ul>

      {installers.length > 0 && (
        <div className="border-t border-rivet px-6 py-4 text-caption text-alu-dim">
          <p className="font-medium text-alu">Installeurs non ajoutés</p>
          <p className="mt-1">
            Ces dossiers contiennent un programme d'installation, pas un jeu installé. Installe-les,
            puis relance la recherche : {installers.map((c) => c.title).join(", ")}.
          </p>
        </div>
      )}
    </Modal>
  );
}

function relative(path: string, dir: string) {
  return path.toLowerCase().startsWith(dir.toLowerCase()) ? path.slice(dir.length + 1) : path;
}
