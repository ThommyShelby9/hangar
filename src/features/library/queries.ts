import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { api, LIBRARY_CHANGED, SESSIONS_CHANGED, statsApi } from "@/lib/api";

export const gamesKey = ["games"] as const;
export const gameKey = (id: number) => ["game", id] as const;

export function useGames() {
  return useQuery({ queryKey: gamesKey, queryFn: api.listGames });
}

export function useGame(id: number) {
  return useQuery({ queryKey: gameKey(id), queryFn: () => api.getGame(id) });
}

/** Ids of the games running right now. */
export function useRunningGames() {
  const { data } = useQuery({ queryKey: ["running"], queryFn: statsApi.runningGames });
  return new Set(data ?? []);
}

export function useGameStats(id: number) {
  return useQuery({ queryKey: ["stats", "game", id], queryFn: () => statsApi.gameStats(id) });
}

/** Refetches data whenever the backend reports a change (scan, artwork, a game starting…). */
export function useLibraryEvents() {
  const queryClient = useQueryClient();
  useEffect(() => {
    const offLibrary = listen(LIBRARY_CHANGED, () => {
      queryClient.invalidateQueries({ queryKey: gamesKey });
      queryClient.invalidateQueries({ queryKey: ["game"] });
    });
    const offSessions = listen(SESSIONS_CHANGED, () => {
      queryClient.invalidateQueries({ queryKey: ["running"] });
      queryClient.invalidateQueries({ queryKey: ["stats"] });
    });
    return () => {
      offLibrary.then((off) => off());
      offSessions.then((off) => off());
    };
  }, [queryClient]);
}
