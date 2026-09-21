import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { api, LIBRARY_CHANGED } from "@/lib/api";

export const gamesKey = ["games"] as const;
export const gameKey = (id: number) => ["game", id] as const;

export function useGames() {
  return useQuery({ queryKey: gamesKey, queryFn: api.listGames });
}

export function useGame(id: number) {
  return useQuery({ queryKey: gameKey(id), queryFn: () => api.getGame(id) });
}

/** Refetches library data whenever the backend reports a change (scan, artwork, launch…). */
export function useLibraryEvents() {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = listen(LIBRARY_CHANGED, () => {
      queryClient.invalidateQueries({ queryKey: gamesKey });
      queryClient.invalidateQueries({ queryKey: ["game"] });
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, [queryClient]);
}
