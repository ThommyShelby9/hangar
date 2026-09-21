import { useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, FolderPlus, X } from "lucide-react";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/Button";
import { api, perfApi, savesApi, startupApi, type PerfSettings, type StartupSettings } from "@/lib/api";
import { toast } from "@/lib/toast";

export function SettingsPage() {
  return (
    <>
      <PageHeader title="Réglages" />
      <div className="max-w-3xl space-y-12 px-10 pb-12">
        <Startup />
        <Performance />
        <Saves />
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

function Startup() {
  const queryClient = useQueryClient();
  const { data: settings } = useQuery({ queryKey: ["startup"], queryFn: startupApi.settings });
  const change = useMutation({
    mutationFn: async (patch: Partial<StartupSettings>) => {
      if (patch.autostart !== undefined) await startupApi.setAutostart(patch.autostart);
      if (patch.hideOnClose !== undefined) await startupApi.setHideOnClose(patch.hideOnClose);
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["startup"] }),
    onError: toast.error,
  });

  return (
    <Section
      title="Démarrage"
      description="Hangar ne compte ton temps de jeu que lorsqu'il est ouvert. Ces options le gardent actif en arrière-plan, près de l'horloge."
    >
      <div className="space-y-3">
        <Toggle
          label="Démarrer Hangar avec Windows (caché près de l'horloge)"
          checked={settings?.autostart ?? false}
          onChange={(autostart) => change.mutate({ autostart })}
        />
        <Toggle
          label="La croix de la fenêtre cache Hangar au lieu de le quitter"
          checked={settings?.hideOnClose ?? true}
          onChange={(hideOnClose) => change.mutate({ hideOnClose })}
        />
      </div>
    </Section>
  );
}

function Performance() {
  const queryClient = useQueryClient();
  const { data: settings } = useQuery({ queryKey: ["perf"], queryFn: perfApi.settings });
  const change = useMutation({
    mutationFn: ({ key, enabled }: { key: keyof PerfSettings; enabled: boolean }) => perfApi.set(key, enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["perf"] }),
    onError: toast.error,
  });

  return (
    <Section
      title="Pendant une partie"
      description="Mode performance, appliqué dès qu'un jeu de ta bibliothèque tourne, qu'il soit lancé depuis Hangar ou non, et défait quand le dernier se ferme."
    >
      <div className="space-y-3">
        <Toggle
          label="Passer Windows en « Meilleures performances », puis remettre le mode d'avant"
          checked={settings?.power ?? true}
          onChange={(enabled) => change.mutate({ key: "power", enabled })}
        />
        <Toggle
          label="Donner au jeu une priorité au-dessus de la normale"
          checked={settings?.priority ?? true}
          onChange={(enabled) => change.mutate({ key: "priority", enabled })}
        />
        <Toggle
          label="Empêcher la mise en veille de l'écran (utile quand on joue à la manette)"
          checked={settings?.awake ?? true}
          onChange={(enabled) => change.mutate({ key: "awake", enabled })}
        />
      </div>
    </Section>
  );
}

function Saves() {
  const queryClient = useQueryClient();
  const { data: settings } = useQuery({ queryKey: ["saves-settings"], queryFn: savesApi.settings });
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["saves-settings"] });
  const install = useMutation({
    mutationFn: savesApi.install,
    onSuccess: () => {
      toast.info("Ludusavi est prêt : tes parties seront sauvegardées à la fin de chaque session.");
      return refresh();
    },
    onError: toast.error,
  });
  const change = useMutation({
    mutationFn: ({ key, value }: { key: "dir" | "auto" | "keep"; value: string }) => savesApi.set(key, value),
    onSuccess: refresh,
    onError: toast.error,
  });
  const chooseDir = async () => {
    const dir = await open({ directory: true, title: "Dossier où ranger les sauvegardes" });
    if (dir) change.mutate({ key: "dir", value: dir });
  };

  return (
    <Section
      title="Sauvegardes des parties"
      description="Hangar copie les sauvegardes de tes jeux à la fin de chaque session, avec Ludusavi, qui sait où plus de 10 000 jeux rangent les leurs. Les jeux passés par un émulateur Steam (CODEX, RUNE…) sont reconnus aussi."
    >
      {!settings ? null : !settings.installed ? (
        <div className="flex flex-wrap items-center gap-4">
          <Button variant="primary" disabled={install.isPending} onClick={() => install.mutate()}>
            {install.isPending ? "Téléchargement…" : "Installer Ludusavi"}
          </Button>
          <p className="text-caption text-alu-dim">
            Version {settings.version}, portable (environ 35 Mo), rangée dans le dossier de Hangar : rien sur C:.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          <Toggle
            label="Sauvegarder automatiquement à la fin de chaque partie"
            checked={settings.auto}
            onChange={(auto) => change.mutate({ key: "auto", value: String(auto) })}
          />
          <dl className="divide-y divide-rivet border-y border-rivet">
            <Row label="Dossier des sauvegardes">
              <span className="flex items-center justify-between gap-4">
                <span className="select-text">{settings.dir}</span>
                <Button size="sm" onClick={chooseDir}>
                  Changer
                </Button>
              </span>
            </Row>
            <Row label="Versions gardées par jeu">
              <select
                value={settings.keep}
                onChange={(e) => change.mutate({ key: "keep", value: e.target.value })}
                className="rounded-control border border-rivet bg-bay px-2 py-1"
              >
                {[3, 5, 10, 20].map((n) => (
                  <option key={n} value={n}>
                    {n} dernières
                  </option>
                ))}
              </select>
            </Row>
          </dl>
        </div>
      )}
    </Section>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-center gap-3">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="size-4 accent-signal"
      />
      {label}
    </label>
  );
}
