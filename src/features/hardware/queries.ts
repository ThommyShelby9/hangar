import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { hardwareApi, type HardwareSnapshot } from "@/lib/api";

export const snapshotKey = ["hardware", "snapshot"] as const;

/** Live readings. Several screens can watch at different paces; the fastest one wins. */
export function useHardwareSnapshot(intervalMs: number) {
  return useQuery({
    queryKey: snapshotKey,
    queryFn: hardwareApi.snapshot,
    refetchInterval: intervalMs,
    refetchIntervalInBackground: false,
  });
}

export function useHardwareInfo() {
  return useQuery({ queryKey: ["hardware", "info"], queryFn: hardwareApi.info, staleTime: 60_000 });
}

const HISTORY = 60;

/** The last 60 values of each series, appended whenever a new snapshot arrives. */
export function useHistory(
  snapshot: HardwareSnapshot | undefined,
  pick: (s: HardwareSnapshot) => Record<string, number | null>,
) {
  const [state, setState] = useState<{ from?: HardwareSnapshot; series: Record<string, number[]> }>(
    {
      series: {},
    },
  );
  // Derived from the incoming snapshot during render, as React recommends over an effect.
  if (snapshot && snapshot !== state.from) {
    const series: Record<string, number[]> = {};
    for (const [key, value] of Object.entries(pick(snapshot))) {
      series[key] = [...(state.series[key] ?? []), value ?? 0].slice(-HISTORY);
    }
    setState({ from: snapshot, series });
  }
  return state.series;
}
