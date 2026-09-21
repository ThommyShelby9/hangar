import { motion } from "motion/react";
import { PageHeader } from "@/components/PageHeader";
import { EmptyState } from "@/components/EmptyState";

export function LibraryPage() {
  return (
    <>
      <PageHeader title="Bibliothèque" />
      <EmptyState title="Le hangar est vide" art={<EmptyStand />}>
        Hangar ira chercher tes jeux sur Steam, sur Epic, dans H:\Games et dans N:\. Tu pourras
        aussi en ajouter à la main en glissant un .exe dans la fenêtre.
      </EmptyState>
    </>
  );
}

/** An empty aircraft stand seen from above: painted lead-in line, stop bar, stand outline. */
function EmptyStand() {
  const draw = { hidden: { pathLength: 0 }, shown: { pathLength: 1 } };
  return (
    <svg viewBox="0 0 240 150" className="mb-8 w-60" aria-hidden="true" fill="none">
      <rect
        x="56"
        y="12"
        width="128"
        height="108"
        rx="2"
        className="stroke-alu-dim/35"
        strokeWidth="1.5"
        strokeDasharray="7 6"
      />
      <motion.path
        d="M 4 146 C 70 146 120 138 120 96 L 120 40"
        className="stroke-signal"
        strokeWidth="3"
        strokeLinecap="round"
        variants={draw}
        initial="hidden"
        animate="shown"
        transition={{ duration: 1.1, ease: [0.3, 0, 0.2, 1] }}
      />
      <motion.line
        x1="100"
        y1="32"
        x2="140"
        y2="32"
        className="stroke-signal"
        strokeWidth="5"
        strokeLinecap="round"
        variants={draw}
        initial="hidden"
        animate="shown"
        transition={{ delay: 1, duration: 0.3 }}
      />
    </svg>
  );
}
