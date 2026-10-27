import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";
import type { Library, User } from "../../api/types";
import { useUi } from "../context";
import { F, G } from "../kit/focusable";
import { Icon, type IconName } from "../kit/icons";
import type { PageKey } from "./navigator";

export interface RailEntry {
  id: string;
  label: string;
  icon: IconName;
  key: PageKey;
}

/** At most this many libraries in the rail. */
const MAX_LIBRARIES = 3;
/** Entries between the avatar and Settings, at most: their pitch fills the rail. */
const MAX_ITEMS = 7;

export interface RailEntries {
  top: RailEntry[];
  settings: RailEntry;
}

/**
 * The rail's entries (the Android app's RailItems): Search, Home, up to
 * three libraries, then Libraries when there are more, Playlists, and
 * Collections when there are three or fewer; Settings at the foot. While
 * music has a queue, Now Playing comes last: added when there is room,
 * else in Collections' place, else in the last library's (which Libraries
 * lists).
 */
export function railEntries(libraries: Library[] | null, nowPlaying = false): RailEntries {
  const libs = libraries ?? [];
  const icon = (l: Library): IconName => (l.kind === "movies" ? "clapperboard" : l.kind === "shows" ? "tv" : l.kind === "music" ? "music" : "film");
  let top: RailEntry[] = [
    { id: "search", label: "Search", icon: "search", key: { name: "search" } },
    { id: "home", label: "Home", icon: "house", key: { name: "home" } },
  ];
  for (const l of libs.slice(0, MAX_LIBRARIES)) top.push({ id: `library:${l.id}`, label: l.name, icon: icon(l), key: { name: "library", id: l.id } });
  if (libs.length > MAX_LIBRARIES) top.push({ id: "libraries", label: "Libraries", icon: "layers", key: { name: "libraries" } });
  top.push({ id: "playlists", label: "Playlists", icon: "list-music", key: { name: "playlists" } });
  if (libs.length > 0 && libs.length <= MAX_LIBRARIES) {
    const first = libs.filter((l) => l.kind !== "music")[0] ?? libs[0]!;
    top.push({ id: "collections", label: "Collections", icon: "layers", key: { name: "collections", libraryId: first.id } });
  }
  if (nowPlaying) {
    if (top.length >= MAX_ITEMS) {
      const lastLibrary = top.filter((e) => e.key.name === "library").pop();
      const out = top.some((e) => e.id === "collections") ? "collections" : lastLibrary?.id;
      top = top.filter((e) => e.id !== out);
    }
    top.push({ id: "now-playing", label: "Now Playing", icon: "audio-lines", key: { name: "nowPlaying" } });
  }
  return { top, settings: { id: "settings", label: "Settings", icon: "settings", key: { name: "settings" } } };
}

/**
 * The rail's entry for the page at the root of the stack (railCurrent):
 * the entry that opened it, Libraries for a library not in the rail, or
 * `null` (sign-in, or a page the rail has no entry for).
 */
export function currentEntry(root: PageKey | null, entries: RailEntries): string | null {
  if (!root) return null;
  const { top } = entries;
  switch (root.name) {
    case "library":
      return (top.find((e) => e.key.name === "library" && e.key.id === root.id) ?? top.find((e) => e.id === "libraries"))?.id ?? null;
    case "collections":
      if (top.some((e) => e.id === "collections")) return "collections";
      return root.libraryId === undefined ? null : currentEntry({ name: "library", id: root.libraryId }, entries);
    case "settings":
      return "settings";
    case "profiles":
      return "profile";
    default:
      return top.find((e) => e.key.name === root.name)?.id ?? null;
  }
}

/** A profile's colour as CSS, or `null` when it isn't one ("#e07a5f" only). */
export function profileColor(color: string | null | undefined): string | null {
  return color && /^#[0-9a-fA-F]{6}$/.test(color) ? color : null;
}

export function initial(name: string): string {
  const c = name.trim().charAt(0);
  return c ? c.toUpperCase() : "?";
}

/**
 * The rail at the left edge: icons alone, and labels on a panel while it
 * has focus (it is open exactly then). Left from a page's first column
 * comes here, to the current entry; Right goes back to where focus was.
 */
/** What the TV says while the server is out of reach: the shell's notice, then the avatar's mark. */
export const OFFLINE_TEXT = "Showing what pley had saved. Reconnecting…";

/** `offline`: the avatar wears the server-off mark (once the shell's notice has had its 6 s). */
export function Rail(p: { entries: RailEntries; me: User | null; current: string | null; offline?: boolean; railRef: (api: RailApi) => void }) {
  const ui = useUi();
  const open = useSignal(false);
  const root = useRef<HTMLDivElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const { top, settings } = p.entries;

  const leave = (restore: boolean) => {
    const el = root.current;
    if (!el) return;
    if (ui.focus.root === el) ui.focus.release(el, restore);
    open.value = false;
  };

  useEffect(() => {
    const api: RailApi = {
      focusCurrent: () => {
        const el = root.current;
        if (!el) return false;
        const marker = el.querySelector(`[data-rail="${p.current ?? "home"}"]`) ?? el.querySelector('[data-rail="home"]');
        const current = marker ? (marker.closest("[data-f]") as HTMLElement | null) : null;
        if (!current) return false;
        if (ui.focus.root !== el) {
          opener.current = ui.focus.current;
          ui.focus.trap(el);
        }
        ui.focus.focus(current);
        open.value = true;
        return true;
      },
      hasFocus: () => !!root.current && ui.focus.root === root.current,
      backToContent: () => leave(true),
    };
    p.railRef(api);
  });

  useEffect(
    () =>
      ui.focus.onChange((el) => {
        const r = root.current;
        if (r && el && !r.contains(el) && open.value) {
          open.value = false;
          if (ui.focus.root === r) ui.focus.release(r, false);
        }
      }),
    [],
  );

  const select = (entry: RailEntry) => {
    leave(false);
    const nav = ui.nav;
    const s = nav.stack.value;
    if (s.length === 1 && JSON.stringify(s[0]!.key) === JSON.stringify(entry.key)) {
      // The page shown already: back to where focus was on it.
      if (opener.current && opener.current.isConnected) ui.focus.focus(opener.current);
      return;
    }
    nav.restart(entry.key);
  };

  const item = (entry: RailEntry) => (
    <F
      key={entry.id}
      class={`rail-item${p.current === entry.id ? " current" : ""}`}
      label={entry.label}
      onSelect={() => select(entry)}
      onKey={(key) => {
        if (key === "right") {
          leave(true);
          return true;
        }
        return key === "left";
      }}
    >
      <span data-rail={entry.id} style="display:contents" />
      <Icon name={entry.icon} size={20} />
      <span class="label t-label one-line">{entry.label}</span>
    </F>
  );

  const me = p.me;
  const color = profileColor(me?.avatar_color);
  return (
    <>
      {open.value && <div class="rail-scrim" />}
      <div ref={root} class={`rail${open.value ? " open" : ""}`}>
        <div class="rail-panel" />
        <div class="rail-edge" />
        <G kind="col" class="rail-items">
          <F
            class={`rail-item avatar${p.current === "profile" ? " current" : ""}`}
            label={p.offline ? `Switch profile. ${OFFLINE_TEXT}` : "Switch profile"}
            onSelect={() => select({ id: "profile", label: "Profiles", icon: "users", key: { name: "profiles" } })}
            onKey={(key) => {
              if (key === "right") {
                leave(true);
                return true;
              }
              return key === "left";
            }}
          >
            <span data-rail="profile" style="display:contents" />
            <div class="rail-avatar t-label" style={color ? `background:${color}` : undefined}>
              <span style="font-weight:700">{initial(me?.name ?? "")}</span>
            </div>
            {p.offline && (
              <div class="offline-mark" aria-hidden="true">
                <Icon name="server-off" size={9} color="var(--fg)" />
              </div>
            )}
            <span class="label" style="top:10px">
              <div class="t-label one-line">{me?.name ?? ""}</div>
              <div class="t-caption one-line" style="opacity:0.8">
                Switch profile
              </div>
            </span>
          </F>
          {top.map(item)}
          <div class="rail-spacer" />
          {item(settings)}
        </G>
      </div>
    </>
  );
}

export interface RailApi {
  /** Focus to the current entry (opening the rail); false when the rail isn't shown. */
  focusCurrent(): boolean;
  hasFocus(): boolean;
  backToContent(): void;
}
