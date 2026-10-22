import { useMutation } from "@tanstack/react-query";
import { FastForward } from "lucide-react";
import { api, errorMessage } from "@/api/client";
import type { ItemDetail, ItemSummary } from "@/api/types";
import { useToast } from "@/components/Toast";
import { IconButton } from "@/components/ui";
import { cn } from "@/lib/cn";

const ANALYZABLE = new Set(["movie", "show", "season", "episode"]);

/** Admin action: find the intro and credits again (the item's whole season for episodes). */
export function AnalyzeMarkersButton({ item }: { item: ItemDetail | ItemSummary }) {
  const toast = useToast();
  const analyze = useMutation({
    mutationFn: (id: number) => api.post<void>(`/api/items/${id}/analyze`),
  });
  if (!ANALYZABLE.has(item.kind)) return null;
  const scope = item.kind === "episode" ? "this season" : item.kind === "show" ? "every season" : "this item";
  return (
    <IconButton
      label="Find intro and credits"
      size="lg"
      className="bg-surface-3/80 hover:bg-border"
      disabled={analyze.isPending}
      onClick={() =>
        analyze.mutate(item.id, {
          onSuccess: () => toast.success(`Looking for intros and credits in ${scope}…`),
          onError: (e) => toast.error(errorMessage(e)),
        })
      }
    >
      <FastForward className={cn("size-5", analyze.isPending && "animate-pulse")} />
    </IconButton>
  );
}
