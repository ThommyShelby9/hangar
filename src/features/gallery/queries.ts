import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { shotsApi } from "@/lib/api";
import { toast } from "@/lib/toast";

/** Read from disk each time the gallery is shown: new captures appear while playing. */
export function useShots() {
  return useQuery({ queryKey: ["shots"], queryFn: shotsApi.list, staleTime: 15_000 });
}

export function useDeleteShot(onDeleted?: () => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: shotsApi.remove,
    onSuccess: () => {
      toast.info("Capture envoyée à la corbeille");
      onDeleted?.();
      return queryClient.invalidateQueries({ queryKey: ["shots"] });
    },
    onError: toast.error,
  });
}
