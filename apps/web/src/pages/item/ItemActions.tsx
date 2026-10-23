import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Ellipsis, FastForward, FolderPlus, ListPlus, RefreshCw } from "lucide-react";
import { api, errorMessage } from "@/api/client";
import { useRefreshItem } from "@/api/hooks";
import { collectable } from "@/api/lists";
import type { ItemDetail } from "@/api/types";
import { useAuth } from "@/auth";
import { useListMenu } from "@/components/lists/ListMenu";
import { Menu, MenuItem, MenuSeparator } from "@/components/Menu";
import { useToast } from "@/components/Toast";
import { IconButton } from "@/components/ui";
import { EditDialogs, editActions, type EditDialog } from "./EditMenu";

/** Kinds whose intro and credits can be detected. */
const ANALYZABLE = new Set(["movie", "show", "season", "episode"]);

/**
 * The "More" menu of an item page: playlists and collections, then (for
 * administrators) editing and maintenance, each with its name.
 */
export function MoreActions({ item }: { item: ItemDetail }) {
  const user = useAuth();
  const lists = useListMenu();
  const toast = useToast();
  const refresh = useRefreshItem();
  const analyze = useMutation({ mutationFn: (id: number) => api.post<void>(`/api/items/${id}/analyze`) });
  const [dialog, setDialog] = useState<EditDialog | null>(null);

  const admin = user.is_admin;
  const edits = admin ? editActions(item) : [];
  const canCollect = admin && collectable(item);
  const canAnalyze = admin && ANALYZABLE.has(item.kind);
  if (!lists && !admin) return null;

  const findMarkers = () => {
    const scope = item.kind === "episode" ? "this season" : item.kind === "show" ? "every season" : "this item";
    analyze.mutate(item.id, {
      onSuccess: () => toast.success(`Looking for intros and credits in ${scope}…`),
      onError: (e) => toast.error(errorMessage(e)),
    });
  };

  return (
    <>
      <Menu
        // Opens leftwards: the button sits at the end of the row (near the edge on phones).
        trigger={({ open, toggle }) => (
          <IconButton
            label="More actions"
            size="lg"
            aria-haspopup="menu"
            aria-expanded={open}
            active={open}
            className="bg-surface-3/80 hover:bg-border"
            onClick={toggle}
          >
            <Ellipsis className="size-5" />
          </IconButton>
        )}
      >
        {(close) => (
          <>
            {lists ? (
              <MenuItem icon={<ListPlus />} onClick={() => (close(), lists.addToPlaylist(item))}>
                Add to playlist…
              </MenuItem>
            ) : null}
            {lists && canCollect ? (
              <MenuItem icon={<FolderPlus />} onClick={() => (close(), lists.addToCollection(item))}>
                Add to collection…
              </MenuItem>
            ) : null}
            {edits.length ? <MenuSeparator /> : null}
            {edits.map((edit) => (
              <MenuItem key={edit.dialog} icon={edit.icon} onClick={() => (close(), setDialog(edit.dialog))}>
                {edit.label}
              </MenuItem>
            ))}
            {admin ? (
              <>
                <MenuSeparator />
                <MenuItem
                  icon={<RefreshCw />}
                  disabled={refresh.isPending}
                  onClick={() => {
                    close();
                    refresh.mutate(item.id, {
                      onSuccess: () => toast.success("Refreshing metadata…"),
                      onError: (e) => toast.error(errorMessage(e)),
                    });
                  }}
                >
                  Refresh metadata
                </MenuItem>
              </>
            ) : null}
            {canAnalyze ? (
              <MenuItem icon={<FastForward />} disabled={analyze.isPending} onClick={() => (close(), findMarkers())}>
                Find intro and credits
              </MenuItem>
            ) : null}
          </>
        )}
      </Menu>
      <EditDialogs item={item} dialog={dialog} onClose={() => setDialog(null)} />
    </>
  );
}
