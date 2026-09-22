import { useState } from "react";
import { Link } from "react-router";
import { HardDrive } from "lucide-react";
import type { Game } from "@/lib/api";
import { formatBytes } from "@/lib/format";
import { useGames } from "@/features/library/queries";

const IDLE_DAYS = 90;
/** Below this, a game isn't worth suggesting for clean-up. */
const WORTH_BYTES = 2 * 1024 ** 3;

/** Where the library's gigabytes are, and the games left aside that take the most. */
export function DiskSpace() {
  const { data: games = [] } = useGames();
  const [now] = useState(() => Date.now() / 1000);
  const measured = games.filter((g): g is Game & { sizeBytes: number } => g.sizeBytes !== null);
  if (!measured.length) return null;

  const drives = new Map<string, number>();
  for (const g of measured) {
    const drive = g.installDir?.slice(0, 2).toUpperCase() ?? "?";
    drives.set(drive, (drives.get(drive) ?? 0) + g.sizeBytes);
  }
  const total = measured.reduce((sum, g) => sum + g.sizeBytes, 0);
  const idle = measured
    .filter((g) => g.sizeBytes >= WORTH_BYTES)
    .filter((g) => (g.lastPlayedAt ? now - g.lastPlayedAt : now - g.addedAt) > IDLE_DAYS * 86_400)
    .sort((a, b) => b.sizeBytes - a.sizeBytes);
  const idleBytes = idle.reduce((sum, g) => sum + g.sizeBytes, 0);

  return (
    <div className="grid max-w-4xl gap-10 lg:grid-cols-2">
      <div>
        <p className="text-alu-dim">
          Ta bibliothèque occupe <span className="text-alu">{formatBytes(total, 0)}</span>.
        </p>
        <ul className="mt-4 space-y-3">
          {[...drives]
            .sort((a, b) => b[1] - a[1])
            .map(([drive, bytes]) => (
              <li key={drive} className="grid grid-cols-[3rem_1fr_5rem] items-center gap-3">
                <span className="flex items-center gap-1.5 text-alu-dim">
                  <HardDrive size={14} /> {drive}
                </span>
                <span className="h-2 overflow-hidden rounded-full bg-rivet/60">
                  <span
                    className="block h-full rounded-full bg-signal"
                    style={{ width: `${(bytes / total) * 100}%` }}
                  />
                </span>
                <span className="text-right tabular-nums">{formatBytes(bytes, 0)}</span>
              </li>
            ))}
        </ul>
      </div>
      {idle.length > 0 && (
        <div>
          <p className="text-alu-dim">
            Pas lancés depuis plus de 3 mois :{" "}
            <span className="text-alu">{formatBytes(idleBytes, 0)}</span> à libérer si tu en as fini
            avec eux.
          </p>
          <ul className="mt-4 divide-y divide-rivet/60">
            {idle.slice(0, 6).map((g) => (
              <li key={g.id} className="flex justify-between gap-4 py-2">
                <Link to={`/jeu/${g.id}`} className="truncate hover:text-signal">
                  {g.title}
                </Link>
                <span className="shrink-0 text-alu-dim tabular-nums">
                  {formatBytes(g.sizeBytes, 0)}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
