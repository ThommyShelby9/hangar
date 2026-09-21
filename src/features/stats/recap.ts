import type { SessionRow } from "@/lib/api";

export interface GameShare {
  gameId: number;
  title: string;
  grid: string | null;
  secs: number;
  sessions: number;
}

export interface Persona {
  name: string;
  detail: string;
}

export interface Recap {
  totalSecs: number;
  sessions: number;
  daysPlayed: number;
  daysInMonth: number;
  games: GameShare[];
  longest: SessionRow | null;
  persona: Persona;
  /** 0 = Monday. */
  bestWeekday: number | null;
  discoveries: string[];
  /** Change against the previous month, in %; null when there's nothing to compare with. */
  trend: number | null;
}

/** When the player plays most: by the hour each session started, weighted by its length. */
const SLOTS: { from: number; to: number; name: string; what: string }[] = [
  { from: 0, to: 5, name: "Oiseau de nuit", what: "entre minuit et 5 h" },
  { from: 5, to: 12, name: "Lève-tôt", what: "le matin" },
  { from: 12, to: 18, name: "Joueur de l'après-midi", what: "l'après-midi" },
  { from: 18, to: 24, name: "Joueur du soir", what: "le soir" },
];

export function monthBounds(year: number, month: number) {
  const from = new Date(year, month, 1);
  const to = new Date(year, month + 1, 1);
  return {
    from: from.getTime() / 1000,
    to: to.getTime() / 1000,
    days: new Date(year, month + 1, 0).getDate(),
  };
}

export function buildRecap(
  sessions: SessionRow[],
  daysInMonth: number,
  previousSecs: number | null,
): Recap {
  const byGame = new Map<number, GameShare>();
  const slotSecs = SLOTS.map(() => 0);
  const weekdaySecs = Array.from({ length: 7 }, () => 0);
  const days = new Set<string>();
  let longest: SessionRow | null = null;
  const discoveries = new Set<string>();

  for (const s of sessions) {
    const game = byGame.get(s.gameId) ?? {
      gameId: s.gameId,
      title: s.title,
      grid: s.grid,
      secs: 0,
      sessions: 0,
    };
    game.secs += s.durationSecs;
    game.sessions += 1;
    byGame.set(s.gameId, game);

    const start = new Date(s.startedAt * 1000);
    const hour = start.getHours();
    slotSecs[SLOTS.findIndex((slot) => hour >= slot.from && hour < slot.to)] += s.durationSecs;
    weekdaySecs[(start.getDay() + 6) % 7] += s.durationSecs;
    days.add(start.toDateString());
    if (!longest || s.durationSecs > longest.durationSecs) longest = s;
    if (s.firstEver) discoveries.add(s.title);
  }

  const totalSecs = sessions.reduce((sum, s) => sum + s.durationSecs, 0);
  const slot = slotSecs.indexOf(Math.max(...slotSecs));
  const share = totalSecs ? Math.round((slotSecs[slot] / totalSecs) * 100) : 0;
  const bestWeekday = totalSecs ? weekdaySecs.indexOf(Math.max(...weekdaySecs)) : null;

  return {
    totalSecs,
    sessions: sessions.length,
    daysPlayed: days.size,
    daysInMonth,
    games: [...byGame.values()].sort((a, b) => b.secs - a.secs),
    longest,
    persona: {
      name: SLOTS[slot].name,
      detail: `${share} % de ton temps de jeu ${SLOTS[slot].what}`,
    },
    bestWeekday,
    discoveries: [...discoveries],
    trend: previousSecs ? Math.round(((totalSecs - previousSecs) / previousSecs) * 100) : null,
  };
}
