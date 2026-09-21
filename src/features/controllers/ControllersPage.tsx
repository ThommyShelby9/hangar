import { PageHeader } from "@/components/PageHeader";
import { EmptyState } from "@/components/EmptyState";

export function ControllersPage() {
  return (
    <>
      <PageHeader title="Manettes" />
      <EmptyState title="Aucune manette configurée">
        Tu pourras tester et régler ta DualShock 4 et ta DualShock 3 ici : boutons, sticks,
        vibrations, barre lumineuse et batterie.
      </EmptyState>
    </>
  );
}
