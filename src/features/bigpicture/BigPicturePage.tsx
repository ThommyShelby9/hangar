import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { AnimatePresence, motion } from "motion/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, imageUrl, manageApi, type Game } from "@/lib/api";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useGames, useRunningGames } from "@/features/library/queries";
import { GameCover } from "@/features/library/GameCover";
import { useControllers } from "@/features/controllers/queries";
import { useNavInput, type NavAction, type PadStyle } from "@/features/bigpicture/useNavInput";

/** A game just started isn't in the "running" list yet: no second launch during this window. */
const LAUNCH_GUARD_MS = 10_000;

interface Row {
  title: string;
  games: Game[];
}

/** Full-screen, controller-first view for the TV. */
export function BigPicturePage() {
  const navigate = useNavigate();
  const { data: games = [] } = useGames();
  const { data: collections = [] } = useQuery({
    queryKey: ["collections"],
    queryFn: manageApi.collections,
  });
  const running = useRunningGames();
  const [pos, setPos] = useState({ row: 0, col: 0 });
  const [message, setMessage] = useState<string | null>(null);

  const rows = useMemo<Row[]>(() => {
    const visible = games.filter((g) => !g.hidden);
    const recent = visible
      .filter((g) => g.lastPlayedAt)
      .sort((a, b) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0))
      .slice(0, 10);
    const all: Row[] = [
      ...(recent.length ? [{ title: "Joués récemment", games: recent }] : []),
      {
        title: "Tous les jeux",
        games: [...visible].sort((a, b) => a.sortTitle.localeCompare(b.sortTitle)),
      },
      ...collections.map((c) => ({
        title: c.name,
        games: visible.filter((g) => g.collections.includes(c.id)),
      })),
    ];
    return all.filter((r) => r.games.length > 0);
  }, [games, collections]);

  const row = Math.min(pos.row, Math.max(0, rows.length - 1));
  const col = Math.min(pos.col, Math.max(0, (rows[row]?.games.length ?? 1) - 1));
  const selected = rows[row]?.games[col];

  // Full screen lasts as long as this screen: leaving it, by any route, restores the window.
  useEffect(() => {
    const appWindow = getCurrentWindow();
    appWindow.setFullscreen(true);
    return () => {
      void leaveFullscreen(appWindow);
    };
  }, []);

  const exit = useCallback(() => navigate("/bibliotheque"), [navigate]);
  const justLaunched = useRef(0);

  const onAction = (action: NavAction) => {
    if (action === "back") return void exit();
    if (!selected) return;
    switch (action) {
      case "left":
        return setPos({ row, col: Math.max(0, col - 1) });
      case "right":
        return setPos({ row, col: Math.min(rows[row].games.length - 1, col + 1) });
      case "up":
        return setPos({ row: Math.max(0, row - 1), col });
      case "down":
        return setPos({ row: Math.min(rows.length - 1, row + 1), col });
      case "favorite":
        return void api.updateGame(selected.id, { favorite: !selected.favorite });
      case "confirm": {
        // The tracker only notices the game a few seconds later; until then, refuse to start it again.
        const busy =
          running.has(selected.id) || performance.now() - justLaunched.current < LAUNCH_GUARD_MS;
        if (busy || selected.status === "missing") return;
        justLaunched.current = performance.now();
        setMessage(`Lancement de ${selected.title}…`);
        api
          .launchGame(selected.id)
          .catch((e) => setMessage(String(e)))
          .finally(() => setTimeout(() => setMessage(null), 4000));
      }
    }
  };
  const detected = useNavInput(onAction);
  // DS4Windows shows a DualShock as an Xbox pad: the buttons in hand are still ✕ ○ △.
  const { data: controllers } = useControllers();
  const padStyle: PadStyle =
    detected === "xbox" && controllers?.ds4windows ? "playstation" : detected;

  // Keep the selected cover in view.
  useEffect(() => {
    document
      .querySelector(`[data-cell="${row}-${col}"]`)
      ?.scrollIntoView({ block: "center", inline: "center", behavior: "smooth" });
  }, [row, col]);

  const hero = imageUrl(selected?.images.hero ?? selected?.images.grid);
  const logo = imageUrl(selected?.images.logo);

  return (
    <div className="fixed inset-0 flex flex-col overflow-hidden bg-tarmac text-alu select-none">
      <AnimatePresence>
        {hero && (
          <motion.img
            key={hero}
            src={hero}
            alt=""
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.5 }}
            className={cn(
              "absolute inset-0 size-full object-cover",
              !selected?.images.hero && "scale-110 blur-2xl",
            )}
          />
        )}
      </AnimatePresence>
      <div className="absolute inset-0 bg-gradient-to-t from-tarmac via-tarmac/80 to-tarmac/20" />
      <div className="absolute inset-0 bg-gradient-to-r from-tarmac/80 via-tarmac/30 to-transparent" />

      <header className="relative flex items-center justify-between px-16 pt-10">
        <span className="font-stencil text-[1.8rem] font-bold tracking-[0.08em]">HANGAR</span>
        <Clock />
      </header>

      <section className="relative flex min-h-[34vh] flex-col justify-end px-16 pb-8">
        {selected && (
          <>
            {logo ? (
              <img
                src={logo}
                alt={selected.title}
                className="max-h-40 max-w-[40vw] object-contain object-left drop-shadow-[0_4px_24px_rgb(0_0_0/0.6)]"
              />
            ) : (
              <h1 className="max-w-[24ch] font-display text-[4.5rem] leading-[0.9] font-bold">
                {selected.title}
              </h1>
            )}
            <p className="mt-5 flex flex-wrap items-center gap-x-8 gap-y-2 text-[1.35rem] text-alu-dim">
              {running.has(selected.id) && (
                <span className="flex items-center gap-2 text-primer">
                  <span className="size-2.5 rounded-full bg-primer" />
                  En cours
                </span>
              )}
              <span>
                {selected.playtimeSecs > 0
                  ? `${formatDuration(selected.playtimeSecs)} de jeu`
                  : "Pas encore joué"}
              </span>
              {selected.favorite && <span className="text-signal">Favori</span>}
              {selected.status === "missing" && <span className="text-flare">Introuvable</span>}
            </p>
            {message && <p className="mt-3 text-[1.35rem]">{message}</p>}
          </>
        )}
      </section>

      <div className="relative min-h-0 flex-1 overflow-hidden">
        <div className="h-full space-y-10 overflow-y-hidden pt-2 pb-24">
          {rows.map((r, ri) => (
            <section key={r.title} className={cn("transition-opacity", ri !== row && "opacity-50")}>
              <h2 className="px-16 font-display text-[1.6rem] font-semibold">{r.title}</h2>
              <ul className="flex gap-6 overflow-x-hidden px-16 py-5">
                {r.games.map((g, ci) => {
                  const active = ri === row && ci === col;
                  return (
                    <li
                      key={g.id}
                      data-cell={`${ri}-${ci}`}
                      onClick={() => setPos({ row: ri, col: ci })}
                      className={cn(
                        "w-44 shrink-0 rounded-[10px] transition-transform duration-200",
                        active && "scale-110",
                      )}
                    >
                      <GameCover
                        game={g}
                        className={cn(
                          "shadow-xl shadow-black/40",
                          active ? "ring-4 ring-signal" : "ring-1 ring-white/10",
                          g.status === "missing" && "opacity-40 grayscale",
                        )}
                      />
                    </li>
                  );
                })}
              </ul>
            </section>
          ))}
        </div>
      </div>

      <footer className="absolute inset-x-0 bottom-0 flex items-center justify-end gap-10 bg-gradient-to-t from-tarmac to-transparent px-16 pt-10 pb-8 text-[1.2rem]">
        <Hint
          style={padStyle}
          button="confirm"
          label={selected && running.has(selected.id) ? "En cours" : "Jouer"}
        />
        <Hint
          style={padStyle}
          button="favorite"
          label={selected?.favorite ? "Retirer des favoris" : "Favori"}
        />
        <Hint style={padStyle} button="back" label="Quitter le mode TV" />
      </footer>
    </div>
  );
}

const GLYPHS: Record<
  "playstation" | "xbox",
  Record<"confirm" | "favorite" | "back", { glyph: string; color: string }>
> = {
  playstation: {
    confirm: { glyph: "✕", color: "#7fa8ff" },
    favorite: { glyph: "△", color: "#4fd1a5" },
    back: { glyph: "○", color: "#ff6b6b" },
  },
  xbox: {
    confirm: { glyph: "A", color: "#6cc24a" },
    favorite: { glyph: "Y", color: "#f2c230" },
    back: { glyph: "B", color: "#e94b3c" },
  },
};
const KEYBOARD = { confirm: "Entrée", favorite: "F", back: "Échap" };

/** Button hint: PlayStation glyphs by default (the user's controllers), Xbox letters for Xbox pads. */
function Hint({
  style,
  button,
  label,
}: {
  style: PadStyle;
  button: "confirm" | "favorite" | "back";
  label: string;
}) {
  const { glyph, color } = GLYPHS[style ?? "playstation"][button];
  return (
    <span className="flex items-center gap-3">
      <span
        aria-hidden
        className="grid size-9 place-items-center rounded-full border-2 text-[1.05rem] font-bold"
        style={{ borderColor: color, color }}
      >
        {glyph}
      </span>
      <span>{label}</span>
      {!style && <span className="text-caption text-alu-dim">({KEYBOARD[button]})</span>}
    </span>
  );
}

function Clock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(timer);
  }, []);
  return (
    <span className="text-[1.6rem] font-semibold tabular-nums">
      {now.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" })}
    </span>
  );
}

/**
 * Windows ignores a "leave full screen" request that arrives while the "enter" transition is still
 * running: ask, check, and ask again.
 */
async function leaveFullscreen(appWindow: ReturnType<typeof getCurrentWindow>) {
  for (let attempt = 0; attempt < 5; attempt++) {
    await appWindow.setFullscreen(false);
    await new Promise((resolve) => setTimeout(resolve, 250));
    if (!(await appWindow.isFullscreen())) return;
  }
}
