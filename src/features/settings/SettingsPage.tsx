import { useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, FolderPlus, X } from "lucide-react";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/Button";
import { api } from "@/lib/api";
import { toast } from "@/lib/toast";

export function SettingsPage() {
  return (
    <>
      <PageHeader title="Réglages" />
      <div className="max-w-3xl space-y-12 px-10 pb-12">
        <WatchedFolders />
        <SteamGridDbKey />
        <ApplicationInfo />
      </div>
    </>
  );
}

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section>
      <h2 className="font-display text-title font-semibold">{title}</h2>
      {description && <p className="mt-1 max-w-[62ch] text-alu-dim">{description}</p>}
      <div className="mt-4">{children}</div>
    </section>
  );
}

function WatchedFolders() {
  const queryClient = useQueryClient();
  const { data: folders = [] } = useQuery({
    queryKey: ["watched-folders"],
    queryFn: api.watchedFolders,
  });
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["watched-folders"] });

  const add = useMutation({
    mutationFn: async () => {
      const path = await open({ directory: true, title: "Dossier qui contient tes jeux" });
      if (path) await api.addWatchedFolder(path);
    },
    onSuccess: refresh,
    onError: toast.error,
  });
  const remove = useMutation({
    mutationFn: api.removeWatchedFolder,
    onSuccess: refresh,
    onError: toast.error,
  });

  return (
    <Section
      title="Dossiers de jeux"
      description="Chaque sous-dossier est analysé quand tu cliques sur « Chercher des jeux » dans la Bibliothèque."
    >
      <ul className="divide-y divide-rivet border-y border-rivet">
        {folders.map((path) => (
          <li key={path} className="flex items-center justify-between py-2.5 pl-1">
            <span className="select-text">{path}</span>
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Ne plus surveiller ${path}`}
              onClick={() => remove.mutate(path)}
            >
              <X size={16} />
            </Button>
          </li>
        ))}
        {folders.length === 0 && <li className="py-3 text-alu-dim">Aucun dossier surveillé.</li>}
      </ul>
      <Button className="mt-3" onClick={() => add.mutate()}>
        <FolderPlus size={16} />
        Ajouter un dossier
      </Button>
    </Section>
  );
}

function SteamGridDbKey() {
  const queryClient = useQueryClient();
  const { data: keySet } = useQuery({ queryKey: ["sgdb-key-set"], queryFn: api.sgdbKeySet });
  const [key, setKey] = useState("");

  const save = useMutation({
    mutationFn: () => api.setSgdbKey(key),
    onSuccess: () => {
      setKey("");
      queryClient.invalidateQueries({ queryKey: ["sgdb-key-set"] });
      toast.info("Clé SteamGridDB enregistrée. Les images arrivent.");
    },
    onError: toast.error,
  });

  return (
    <Section
      title="SteamGridDB"
      description={
        <>
          Fournit les jaquettes, fonds, logos et icônes. La clé se crée gratuitement sur
          steamgriddb.com, dans Préférences puis API. Elle reste sur ce PC.
        </>
      }
    >
      {keySet && (
        <p className="mb-3 flex items-center gap-2 text-primer">
          <Check size={16} />
          Clé enregistrée
        </p>
      )}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
        className="flex gap-2"
      >
        <input
          type="password"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder={keySet ? "Remplacer la clé" : "Colle ta clé d'API ici"}
          aria-label="Clé d'API SteamGridDB"
          autoComplete="off"
          className="h-9 flex-1 rounded-control border border-rivet bg-bay px-3 placeholder:text-alu-dim/60 focus:border-signal focus:outline-none"
        />
        <Button type="submit" variant="primary" disabled={!key.trim() || save.isPending}>
          {save.isPending ? "Vérification…" : "Enregistrer"}
        </Button>
      </form>
    </Section>
  );
}

function ApplicationInfo() {
  const { data: info, error } = useQuery({ queryKey: ["app-info"], queryFn: api.appInfo });

  return (
    <Section title="Application">
      {info?.onSystemDrive && (
        <p className="mb-3 text-flare">
          Hangar est sur le disque C:. Déplace son dossier vers un autre disque, par exemple
          O:\Apps\Hangar, pour garder C: libre.
        </p>
      )}
      {error ? (
        <p className="text-flare">Impossible de lire les informations : {String(error)}</p>
      ) : (
        <dl className="divide-y divide-rivet border-y border-rivet">
          <Row label="Dossier des données">
            <span className="select-text">{info?.dataDir ?? "…"}</span>
          </Row>
          <Row label="Version">{info?.version ?? "…"}</Row>
          <Row label="Base de données">{info ? `Schéma v${info.schemaVersion}` : "…"}</Row>
        </dl>
      )}
    </Section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[12rem_1fr] gap-4 py-3">
      <dt className="text-alu-dim">{label}</dt>
      <dd className="break-all tabular-nums">{children}</dd>
    </div>
  );
}
