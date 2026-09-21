import { Link } from "react-router";
import { Heart } from "lucide-react";
import type { Game } from "@/lib/api";
import { GameCover } from "@/features/library/GameCover";
import { cn } from "@/lib/utils";

export function GameCard({ game }: { game: Game }) {
  const missing = game.status === "missing";
  return (
    <Link
      to={`/jeu/${game.id}`}
      className="group block rounded-[10px] focus-visible:outline-offset-4"
      draggable={false}
    >
      <div className="relative">
        <GameCover
          game={game}
          className={cn(
            "ring-1 ring-white/5 transition duration-200 group-hover:-translate-y-1 group-hover:ring-2 group-hover:ring-signal/80",
            missing && "opacity-40 grayscale",
          )}
        />
        {game.favorite && (
          <Heart
            size={16}
            aria-label="Favori"
            className="absolute top-2.5 right-2.5 fill-signal text-signal drop-shadow"
          />
        )}
      </div>
      <p className="mt-2.5 truncate text-alu-dim transition-colors group-hover:text-alu">
        {game.title}
      </p>
      {missing && <p className="text-caption text-flare">Introuvable</p>}
    </Link>
  );
}
