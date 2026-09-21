import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { motion } from "motion/react";
import { ChevronLeft, ChevronRight, FolderOpen, ImageUp, Trash2, X } from "lucide-react";
import { manageApi, shotsApi, type Shot } from "@/lib/api";
import { Button } from "@/components/Button";
import { formatDateTime } from "@/lib/format";
import { toast } from "@/lib/toast";
import { useDeleteShot } from "@/features/gallery/queries";

/** Full-screen viewer: ← → to browse, Esc to close. */
export function Lightbox({
  shots,
  index,
  onIndex,
  onClose,
  titleOf,
}: {
  shots: Shot[];
  index: number;
  onIndex: (index: number) => void;
  onClose: () => void;
  titleOf: (shot: Shot) => string;
}) {
  const shot = shots[index];
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const remove = useDeleteShot(() => {
    if (shots.length <= 1) onClose();
    else if (index >= shots.length - 1) onIndex(index - 1);
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowLeft" && index > 0) onIndex(index - 1);
      if (e.key === "ArrowRight" && index < shots.length - 1) onIndex(index + 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [index, shots.length, onIndex, onClose]);

  if (!shot) return null;
  const confirming = confirmDelete === shot.path;

  const useAsBackground = async () => {
    if (shot.gameId === null) return;
    try {
      await manageApi.setImageFile(shot.gameId, "hero", shot.path);
      toast.info(`Nouveau fond pour ${titleOf(shot)}`);
    } catch (e) {
      toast.error(e);
    }
  };

  return createPortal(
    <motion.div
      role="dialog"
      aria-modal="true"
      aria-label="Capture"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      className="fixed inset-0 z-50 flex flex-col bg-black/92 backdrop-blur-sm"
    >
      <header className="flex items-center gap-4 px-6 py-3 text-alu">
        <div className="min-w-0 flex-1">
          <p className="truncate font-display text-title font-semibold">{titleOf(shot)}</p>
          <p className="text-caption text-alu-dim tabular-nums">
            {formatDateTime(shot.takenAt)} · {index + 1} / {shots.length}
          </p>
        </div>
        {shot.gameId !== null && (
          <Button size="sm" onClick={useAsBackground}>
            <ImageUp size={15} /> Utiliser comme fond du jeu
          </Button>
        )}
        <Button size="sm" onClick={() => shotsApi.reveal(shot.path).catch(toast.error)}>
          <FolderOpen size={15} /> Voir dans l'explorateur
        </Button>
        <Button
          size="sm"
          variant="danger"
          disabled={remove.isPending}
          onClick={() => (confirming ? remove.mutate(shot.path) : setConfirmDelete(shot.path))}
        >
          <Trash2 size={15} /> {confirming ? "Confirmer" : "Supprimer"}
        </Button>
        <Button size="icon" variant="ghost" aria-label="Fermer" onClick={onClose}>
          <X size={20} />
        </Button>
      </header>

      <div className="relative flex min-h-0 flex-1 items-center justify-center px-16 pb-6">
        <motion.img
          key={shot.path}
          src={shotsApi.url(shot.path)}
          alt={titleOf(shot)}
          initial={{ opacity: 0, scale: 0.985 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.18 }}
          className="max-h-full max-w-full rounded-control object-contain shadow-2xl shadow-black"
        />
        <NavButton side="left" disabled={index === 0} onClick={() => onIndex(index - 1)} />
        <NavButton side="right" disabled={index >= shots.length - 1} onClick={() => onIndex(index + 1)} />
      </div>
    </motion.div>,
    document.body,
  );
}

function NavButton({ side, disabled, onClick }: { side: "left" | "right"; disabled: boolean; onClick: () => void }) {
  const Icon = side === "left" ? ChevronLeft : ChevronRight;
  return (
    <button
      type="button"
      aria-label={side === "left" ? "Capture précédente" : "Capture suivante"}
      disabled={disabled}
      onClick={onClick}
      className={`absolute top-1/2 ${side === "left" ? "left-3" : "right-3"} -translate-y-1/2 rounded-full p-2 text-alu/70 transition-colors hover:bg-white/10 hover:text-alu disabled:invisible`}
    >
      <Icon size={32} />
    </button>
  );
}
