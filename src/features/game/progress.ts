import type { Progress } from "@/lib/api";

export const PROGRESS_LABEL: Record<Progress, string> = {
  todo: "À commencer",
  playing: "En cours",
  done: "Terminé",
  completed: "100 %",
  dropped: "Abandonné",
};
