import type { Shot } from "@/lib/api";
import { shotsApi } from "@/lib/api";
import { formatShortDate } from "@/lib/format";

export function ShotGrid({
  shots,
  titleOf,
  onOpen,
}: {
  shots: Shot[];
  /** Shown on each tile when captures of several games are mixed. */
  titleOf?: (shot: Shot) => string;
  onOpen: (index: number) => void;
}) {
  return (
    <ul className="grid grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-3">
      {shots.map((shot, i) => (
        <li key={shot.path}>
          <button
            type="button"
            onClick={() => onOpen(i)}
            className="group relative block aspect-video w-full overflow-hidden rounded-control bg-bay ring-signal outline-none focus-visible:ring-2"
          >
            <img
              src={shotsApi.thumbUrl(shot.path)}
              alt=""
              loading="lazy"
              decoding="async"
              className="size-full object-cover transition-transform duration-300 group-hover:scale-[1.04]"
            />
            <span className="absolute inset-x-0 bottom-0 flex items-end justify-between gap-2 bg-gradient-to-t from-black/80 to-transparent px-3 pt-6 pb-2 text-left text-caption text-alu opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
              <span className="truncate font-medium">{titleOf?.(shot)}</span>
              <span className="shrink-0 text-alu/75 tabular-nums">{formatShortDate(shot.takenAt)}</span>
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
