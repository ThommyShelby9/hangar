import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router";
import { motion } from "motion/react";
import { Dices, Play, X } from "lucide-react";
import { api, type Game } from "@/lib/api";
import { Button } from "@/components/Button";
import { GameCover } from "@/features/library/GameCover";
import { formatDuration } from "@/lib/format";
import { toast } from "@/lib/toast";

const CARD = 168; // cover width in the reel, gap included
const SPIN_CARDS = 34;
const SPIN_SECS = 3.4;

/** Favours games left aside for a while, and games never started. */
function weight(game: Game, now = Date.now() / 1000): number {
  const idleDays = game.lastPlayedAt ? (now - game.lastPlayedAt) / 86_400 : null;
  let w = idleDays === null ? 4 : 1 + Math.min(idleDays, 90) / 15;
  if (game.favorite) w *= 1.3;
  if (game.progress === "done" || game.progress === "completed" || game.progress === "dropped")
    w *= 0.15;
  return w;
}

function draw(games: Game[]): Game {
  const total = games.reduce((sum, g) => sum + weight(g), 0);
  let r = Math.random() * total;
  for (const g of games) {
    r -= weight(g);
    if (r <= 0) return g;
  }
  return games[games.length - 1];
}

/** "Je lance quoi ?": a slot-machine reel of covers that stops on tonight's game. */
export function Roulette({ games, onClose }: { games: Game[]; onClose: () => void }) {
  const candidates = useMemo(() => games.filter((g) => !g.hidden && g.status === "ok"), [games]);
  const [spin, setSpin] = useState(() => newSpin(candidates));
  const [landed, setLanded] = useState(false);
  const navigate = useNavigate();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const again = () => {
    setLanded(false);
    setSpin(newSpin(candidates, spin.key + 1));
  };
  const play = async () => {
    try {
      await api.launchGame(spin.winner.id);
      onClose();
    } catch (e) {
      toast.error(e);
    }
  };

  return createPortal(
    <motion.div
      role="dialog"
      aria-modal="true"
      aria-label="Je lance quoi ?"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      className="fixed inset-0 z-50 flex flex-col items-center justify-center gap-8 bg-black/85 backdrop-blur-md"
    >
      <button
        type="button"
        aria-label="Fermer"
        onClick={onClose}
        className="absolute top-5 right-5 rounded-full p-2 text-alu/70 hover:bg-white/10 hover:text-alu"
      >
        <X size={22} />
      </button>
      <p className="font-display text-[2rem] font-bold tracking-[0.02em] text-alu">
        Ce soir, ce sera…
      </p>

      <div className="relative w-full overflow-hidden py-4">
        {/* Painted runway marker: where the reel stops. */}
        <div className="pointer-events-none absolute inset-y-0 left-1/2 z-10 w-[152px] -translate-x-1/2 rounded-[10px] border-2 border-signal shadow-[0_0_40px_rgba(244,182,63,0.35)]" />
        <motion.div
          key={spin.key}
          className="flex gap-4"
          style={{ paddingLeft: `calc(50% - ${CARD / 2 - 8}px)` }}
          initial={{ x: 0 }}
          animate={{ x: -spin.stop * CARD }}
          transition={{ duration: SPIN_SECS, ease: [0.12, 0.8, 0.18, 1] }}
          onAnimationComplete={() => setLanded(true)}
        >
          {spin.reel.map((g, i) => (
            <GameCover key={i} game={g} className="w-[152px] shrink-0 shadow-xl shadow-black/60" />
          ))}
        </motion.div>
      </div>

      <div className="flex h-28 flex-col items-center gap-4">
        {landed && (
          <motion.div
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            className="flex flex-col items-center gap-4"
          >
            <p className="text-center">
              <span className="font-display text-title font-semibold text-alu">
                {spin.winner.title}
              </span>
              <span className="block text-alu-dim">{subtitle(spin.winner)}</span>
            </p>
            <div className="flex gap-3">
              <Button variant="primary" size="lg" onClick={play}>
                <Play size={20} /> Jouer
              </Button>
              <Button onClick={again}>
                <Dices size={16} /> Encore
              </Button>
              <Button
                variant="ghost"
                onClick={() => {
                  onClose();
                  navigate(`/jeu/${spin.winner.id}`);
                }}
              >
                Voir la fiche
              </Button>
            </div>
          </motion.div>
        )}
      </div>
    </motion.div>,
    document.body,
  );
}

function newSpin(games: Game[], key = 0) {
  const winner = draw(games);
  const reel = Array.from(
    { length: SPIN_CARDS },
    () => games[Math.floor(Math.random() * games.length)],
  );
  reel.push(winner);
  // A few covers after the winner, so the reel doesn't end at the marker.
  const after = Array.from({ length: 5 }, () => games[Math.floor(Math.random() * games.length)]);
  return { key, winner, reel: [...reel, ...after], stop: reel.length - 1 };
}

function subtitle(game: Game): string {
  if (!game.lastPlayedAt) return "Jamais lancé : c'est le moment.";
  const days = Math.floor((Date.now() / 1000 - game.lastPlayedAt) / 86_400);
  const played = game.playtimeSecs ? `${formatDuration(game.playtimeSecs)} au compteur` : "";
  const when =
    days < 1 ? "joué aujourd'hui" : days === 1 ? "joué hier" : `pas lancé depuis ${days} jours`;
  return [played, when].filter(Boolean).join(" · ");
}
