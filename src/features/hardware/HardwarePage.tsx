import { PageHeader } from "@/components/PageHeader";
import { EmptyState } from "@/components/EmptyState";

export function HardwarePage() {
  return (
    <>
      <PageHeader title="Matériel" />
      <EmptyState title="Le moniteur n'est pas encore branché">
        Processeur, cartes graphiques (GeForce MX250 et Iris Plus), mémoire, températures et
        batterie, en direct.
      </EmptyState>
    </>
  );
}
