import { useRef, type PointerEvent } from "react";
import { Link } from "react-router";
import { motion } from "motion/react";
import { Heart } from "lucide-react";
import type { Game } from "@/lib/api";
import { GameCover } from "@/features/library/GameCover";
import { cn } from "@/lib/utils";

const MAX_TILT = 9; // degrees

export function GameCard({ game, running }: { game: Game; running?: boolean }) {
  const missing = game.status === "missing";
  const tilt = useRef<HTMLDivElement>(null);

  // The card leans towards the pointer and a highlight follows it, like light on a printed sleeve.
  const onMove = (e: PointerEvent) => {
    const el = tilt.current;
    if (!el || e.pointerType !== "mouse") return;
    const r = el.getBoundingClientRect();
    const x = (e.clientX - r.left) / r.width;
    const y = (e.clientY - r.top) / r.height;
    el.style.setProperty("--ry", `${(x - 0.5) * 2 * MAX_TILT}deg`);
    el.style.setProperty("--rx", `${(0.5 - y) * 2 * MAX_TILT}deg`);
    el.style.setProperty("--mx", `${x * 100}%`);
    el.style.setProperty("--my", `${y * 100}%`);
  };
  const onLeave = () => {
    const el = tilt.current;
    el?.style.setProperty("--rx", "0deg");
    el?.style.setProperty("--ry", "0deg");
  };

  return (
    <Link
      to={`/jeu/${game.id}`}
      className="group block rounded-[10px] focus-visible:outline-offset-4"
      draggable={false}
    >
      <motion.div
        layoutId={`cover-${game.id}`}
        transition={{ type: "spring", stiffness: 260, damping: 30 }}
        className="relative [perspective:700px]"
      >
        <div
          ref={tilt}
          onPointerMove={onMove}
          onPointerLeave={onLeave}
          className="relative [transform:rotateX(var(--rx,0deg))_rotateY(var(--ry,0deg))] transition-transform duration-200 ease-out group-hover:duration-75 motion-reduce:[transform:none]"
        >
          <GameCover
            game={game}
            className={cn(
              "ring-1 ring-white/5 transition-shadow duration-200 group-hover:shadow-xl group-hover:ring-2 group-hover:shadow-black/50 group-hover:ring-signal/80",
              running && "ring-2 ring-primer",
              missing && "opacity-40 grayscale",
            )}
          />
          {/* Moving highlight. */}
          <div
            aria-hidden
            className="pointer-events-none absolute inset-0 rounded-[8px] opacity-0 transition-opacity duration-200 group-hover:opacity-100 motion-reduce:hidden"
            style={{
              background:
                "radial-gradient(circle at var(--mx, 50%) var(--my, 0%), rgb(255 255 255 / 0.22), transparent 55%)",
            }}
          />
          {game.favorite && (
            <Heart
              size={16}
              aria-label="Favori"
              className="absolute top-2.5 right-2.5 fill-signal text-signal drop-shadow"
            />
          )}
          {running && (
            <span className="absolute bottom-2.5 left-2.5 flex items-center gap-1.5 rounded-full bg-tarmac/90 px-2.5 py-1 text-caption text-primer">
              <span className="size-1.5 rounded-full bg-primer" />
              En cours
            </span>
          )}
        </div>
      </motion.div>
      <p className="mt-2.5 truncate text-alu-dim transition-colors group-hover:text-alu">
        {game.title}
      </p>
      {missing && <p className="text-caption text-flare">Introuvable</p>}
    </Link>
  );
}
