import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { PageHeader } from "@/components/PageHeader";
import { api } from "@/lib/api";

export function SettingsPage() {
  const { data: info, error } = useQuery({ queryKey: ["app-info"], queryFn: api.appInfo });

  return (
    <>
      <PageHeader title="Réglages" />
      <section className="px-10">
        <h2 className="font-display text-title font-semibold">Application</h2>
        {info?.onSystemDrive && (
          <p className="mt-3 max-w-2xl text-flare">
            Hangar est sur le disque C:. Déplace son dossier vers un autre disque, par exemple
            O:\Apps\Hangar, pour garder C: libre.
          </p>
        )}
        {error ? (
          <p className="mt-3 text-flare">Impossible de lire les informations : {String(error)}</p>
        ) : (
          <dl className="mt-4 max-w-2xl divide-y divide-rivet border-y border-rivet">
            <Row label="Dossier des données">
              <span className="select-text">{info?.dataDir ?? "…"}</span>
            </Row>
            <Row label="Version">{info?.version ?? "…"}</Row>
            <Row label="Base de données">{info ? `Schéma v${info.schemaVersion}` : "…"}</Row>
          </dl>
        )}
      </section>
    </>
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
