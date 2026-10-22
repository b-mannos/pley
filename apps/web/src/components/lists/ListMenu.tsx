import { createContext, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Eye, EyeOff, FolderPlus, ListPlus, Play, Shuffle } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useSetWatched } from "@/api/hooks";
import { collectable } from "@/api/lists";
import type { ItemSummary } from "@/api/types";
import { useAuth } from "@/auth";
import { MenuItem, MenuSeparator } from "@/components/Menu";
import { useToast } from "@/components/Toast";
import { useDismiss } from "@/lib/hooks";
import { usePlayItem } from "@/lib/usePlayItem";
import { AddToCollectionModal } from "./AddToCollectionModal";
import { AddToPlaylistModal } from "./AddToPlaylistModal";

interface ListMenuApi {
  /** The right-click menu of a card, at viewport coordinates. */
  openMenu: (item: ItemSummary, x: number, y: number) => void;
  addToPlaylist: (item: ItemSummary) => void;
  /** Administrators, movies and shows. */
  addToCollection: (item: ItemSummary) => void;
}

const ListMenuContext = createContext<ListMenuApi | null>(null);

/** `null` outside <ListMenuProvider> (cards then keep the browser's menu). */
export function useListMenu(): ListMenuApi | null {
  return useContext(ListMenuContext);
}

/** Card context menus and the "Add to playlist / collection" dialogs. */
export function ListMenuProvider({ children }: { children: ReactNode }) {
  const [menu, setMenu] = useState<{ item: ItemSummary; x: number; y: number } | null>(null);
  const [forPlaylist, setForPlaylist] = useState<ItemSummary | null>(null);
  const [forCollection, setForCollection] = useState<ItemSummary | null>(null);
  const api = useMemo<ListMenuApi>(
    () => ({
      openMenu: (item, x, y) => setMenu({ item, x, y }),
      addToPlaylist: setForPlaylist,
      addToCollection: setForCollection,
    }),
    [],
  );

  return (
    <ListMenuContext.Provider value={api}>
      {children}
      {menu ? <CardMenu key={`${menu.item.id}:${menu.x}:${menu.y}`} {...menu} api={api} onClose={() => setMenu(null)} /> : null}
      {/* Keyed so a dialog starts blank for every item. */}
      <AddToPlaylistModal key={`p${forPlaylist?.id ?? 0}`} item={forPlaylist} onClose={() => setForPlaylist(null)} />
      <AddToCollectionModal key={`c${forCollection?.id ?? 0}`} item={forCollection} onClose={() => setForCollection(null)} />
    </ListMenuContext.Provider>
  );
}

function isWatched(item: ItemSummary): boolean {
  if (item.kind === "show" || item.kind === "season") {
    return item.leaf_count != null && item.leaf_count > 0 && (item.viewed_leaf_count ?? 0) >= item.leaf_count;
  }
  return item.user_state.watched;
}

function CardMenu({ item, x, y, api, onClose }: { item: ItemSummary; x: number; y: number; api: ListMenuApi; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });
  const user = useAuth();
  const play = usePlayItem();
  const setWatched = useSetWatched();
  const toast = useToast();
  useDismiss(ref, true, onClose);

  // Keep the menu inside the viewport.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    setPos({
      left: Math.max(8, Math.min(x, window.innerWidth - width - 8)),
      top: Math.max(8, Math.min(y, window.innerHeight - height - 8)),
    });
  }, [x, y]);

  useEffect(() => {
    const close = () => onClose();
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
    };
  }, [onClose]);

  const run = (action: () => void) => () => {
    onClose();
    action();
  };
  const music = item.kind === "artist" || item.kind === "album";
  const watchable = item.kind === "movie" || item.kind === "episode" || item.kind === "show" || item.kind === "season";
  const watched = isWatched(item);

  return createPortal(
    <div
      ref={ref}
      role="menu"
      aria-label={item.title}
      style={{ left: pos.left, top: pos.top }}
      onContextMenu={(e) => e.preventDefault()}
      className="fixed z-[55] min-w-52 max-w-72 animate-pop-in overflow-hidden rounded-lg border border-border bg-surface-2 py-1 shadow-xl shadow-black/50"
    >
      <div className="truncate px-3.5 pb-1.5 pt-1 text-xs font-semibold text-subtle">{item.title}</div>
      <MenuItem icon={<Play />} onClick={run(() => void play(item))}>
        Play
      </MenuItem>
      {music ? (
        <MenuItem icon={<Shuffle />} onClick={run(() => void play(item, { shuffle: true }))}>
          Shuffle
        </MenuItem>
      ) : null}
      <MenuItem icon={<ListPlus />} onClick={run(() => api.addToPlaylist(item))}>
        Add to playlist…
      </MenuItem>
      {user.is_admin && collectable(item) ? (
        <MenuItem icon={<FolderPlus />} onClick={run(() => api.addToCollection(item))}>
          Add to collection…
        </MenuItem>
      ) : null}
      {watchable ? (
        <>
          <MenuSeparator />
          <MenuItem
            icon={watched ? <EyeOff /> : <Eye />}
            onClick={run(() =>
              setWatched.mutate({ id: item.id, watched: !watched }, { onError: (err) => toast.error(errorMessage(err)) }),
            )}
          >
            {watched ? "Mark as unwatched" : "Mark as watched"}
          </MenuItem>
        </>
      ) : null}
    </div>,
    document.body,
  );
}
