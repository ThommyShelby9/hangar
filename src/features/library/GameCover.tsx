import { useState } from "react";
import { imageUrl, type Game } from "@/lib/api";
import { cn } from "@/lib/utils";

/** Vertical cover, or a generated tile with the title when there is no artwork yet. */
export function GameCover({ game, className }: { game: Game; className?: string }) {
  const [failed, setFailed] = useState(false);
  const src = imageUrl(game.images.grid);

  return (
    <div
      className={cn("relative aspect-[2/3] overflow-hidden rounded-[8px] bg-bay", className)}
      style={src && !failed ? undefined : { background: fallbackBackground(game.title) }}
    >
      {src && !failed ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          draggable={false}
          onError={() => setFailed(true)}
          className="size-full object-cover"
        />
      ) : (
        <div className="flex size-full items-end p-4">
          <span className="font-display text-[1.6rem] leading-[0.95] font-bold [overflow-wrap:anywhere] text-alu">
            {game.title}
          </span>
        </div>
      )}
    </div>
  );
}

/** A dusk-tinted gradient whose hue is derived from the title, stable across launches. */
function fallbackBackground(title: string) {
  let hash = 0;
  for (const char of title) hash = (hash * 31 + char.charCodeAt(0)) | 0;
  const hue = Math.abs(hash) % 360;
  return `linear-gradient(160deg, oklch(0.42 0.06 ${hue}) 0%, oklch(0.24 0.03 ${hue}) 100%)`;
}
