import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import {
  CONTROLLER_INPUT,
  CONTROLLERS_CHANGED,
  controllersApi,
  type ControllersStatus,
  type PadInput,
} from "@/lib/api";

const statusKey = ["controllers"] as const;

/** Connected controllers, kept current by the backend's change events. */
export function useControllers() {
  const queryClient = useQueryClient();
  useEffect(() => {
    const off = listen<ControllersStatus>(CONTROLLERS_CHANGED, (e) =>
      queryClient.setQueryData(statusKey, e.payload),
    );
    return () => {
      off.then((f) => f());
    };
  }, [queryClient]);
  return useQuery({ queryKey: statusKey, queryFn: controllersApi.status });
}

/** Live inputs (~30/s), streamed only while a component using this hook is mounted. */
export function useLiveInputs() {
  const [inputs, setInputs] = useState<Record<number, PadInput>>({});
  useEffect(() => {
    controllersApi.live(true);
    const off = listen<PadInput[]>(CONTROLLER_INPUT, (e) =>
      setInputs(Object.fromEntries(e.payload.map((p) => [p.id, p]))),
    );
    return () => {
      controllersApi.live(false);
      off.then((f) => f());
    };
  }, []);
  return inputs;
}
