import { PageHeader } from "@/components/PageHeader";
import { EmptyState } from "@/components/EmptyState";

export function StatsPage() {
  return (
    <>
      <PageHeader title="Stats" />
      <EmptyState title="Aucune session enregistrée">
        Le temps passé sur chaque jeu s'affichera ici dès ta première partie, même si tu lances le
        jeu sans passer par Hangar.
      </EmptyState>
    </>
  );
}
