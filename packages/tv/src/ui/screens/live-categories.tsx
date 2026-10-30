import { useLayoutEffect, useMemo, useState } from "preact/hooks";
import type { LiveGroups } from "../../api/types";
import { categoryKey, groupTree, type LiveCategory } from "../../live/data";
import { useUi } from "../context";
import { F, G } from "../kit/focusable";
import { Icon, type IconName } from "../kit/icons";
import { useTrap } from "../kit/panel";

/** A line's pitch in the list, and the lines drawn either side of the focused one. */
const PITCH = 44;
const DRAWN = 12;
/** Where the list's lines start, under its title. */
const LIST_TOP = 78;

/** 57176 → "57,176". */
export function count(n: number): string {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

type Line =
  | { kind: "category"; key: string; category: LiveCategory; label: string; icon: IconName; count: number | null }
  | { kind: "heading"; key: string; label: string; count: number }
  | { kind: "region"; key: string; region: string; groups: number; count: number; open: boolean }
  | { kind: "group"; key: string; category: LiveCategory; label: string; region: string | null; count: number; inRegion: boolean };

/** The list's lines: the marked categories and every channel, then the groups, a region's under it when it is open. */
export function categoryLines(groups: LiveGroups | null, open: Record<string, boolean>): Line[] {
  const lines: Line[] = [
    { kind: "category", key: "favorites", category: { kind: "favorites" }, label: "Favourites", icon: "star", count: groups ? groups.favorites : null },
    { kind: "category", key: "recent", category: { kind: "recent" }, label: "Recently watched", icon: "history", count: groups ? groups.recent : null },
    { kind: "category", key: "all", category: { kind: "all" }, label: "All channels", icon: "layers", count: groups ? groups.total : null },
  ];
  if (!groups || groups.groups.length < 2) return lines;
  lines.push({ kind: "heading", key: "heading", label: "Groups", count: groups.groups.length });
  for (const e of groupTree(groups.groups)) {
    if (e.kind === "group") {
      lines.push({ kind: "group", key: `group:${e.group.name}`, category: { kind: "group", group: e.group.name }, label: e.group.name, region: null, count: e.group.count, inRegion: false });
      continue;
    }
    const isOpen = !!open[e.region];
    lines.push({ kind: "region", key: `region:${e.region}`, region: e.region, groups: e.groups.length, count: e.count, open: isOpen });
    if (isOpen) {
      for (const g of e.groups) lines.push({ kind: "group", key: `group:${g.name}`, category: { kind: "group", group: g.name }, label: g.label, region: g.region, count: g.count, inRegion: true });
    }
  }
  return lines;
}

/**
 * Live TV's categories, as TiviMate lists them, in a panel at the left:
 * Favourites, Recently watched and All channels, then the provider's
 * groups, those of a region gathered under it (OK opens a region). The
 * one shown has focus as it opens; OK on another shows it; Back or Right
 * closes the panel. Only the lines near the focused one are drawn.
 */
export function CategoryPanel(p: { groups: LiveGroups | null; selected: LiveCategory; left: number; onSelect: (c: LiveCategory) => void; onClose: () => void }) {
  const ui = useUi();
  const trap = useTrap();
  const selectedKey = categoryKey(p.selected);
  const selectedRegion = p.selected.kind === "group" ? (p.groups?.groups.filter((g) => g.name === (p.selected as { group: string }).group)[0]?.region ?? null) : null;
  const [open, setOpen] = useState<Record<string, boolean>>(() => (selectedRegion ? { [selectedRegion]: true } : {}));
  const lines = useMemo(() => categoryLines(p.groups, open), [p.groups, open]);
  const startAt = Math.max(0, lines.findIndex((l) => l.key === selectedKey));
  const [at, setAt] = useState(startAt);
  const [inside, setInside] = useState(false);
  useLayoutEffect(() => {
    const t = setTimeout(() => setInside(true), 16);
    const off = ui.input.push((key, e) => {
      if (key === "back" || key === "right") {
        if (e.type === "down" && !e.repeat) p.onClose();
        return true;
      }
      return false;
    });
    return () => {
      clearTimeout(t);
      off();
    };
  }, []);

  const height = 540 - LIST_TOP;
  const shift = Math.max(0, Math.min(lines.length * PITCH - height + 12, at * PITCH - height * 0.35));
  const rows = [];
  for (let i = Math.max(0, at - DRAWN); i < Math.min(lines.length, at + DRAWN + 4); i++) {
    const l = lines[i]!;
    const top = `top:${i * PITCH}px`;
    if (l.kind === "heading") {
      rows.push(
        <div key={l.key} class="live-cat-heading t-caption" style={top}>
          <span>{l.label}</span>
          <span>{count(l.count)}</span>
        </div>,
      );
      continue;
    }
    const current = l.kind !== "region" && l.key === selectedKey;
    const holds = l.kind === "region" && !l.open && selectedRegion === l.region;
    rows.push(
      <F
        key={l.key}
        class={`live-cat${current || holds ? " current" : ""}${l.kind === "group" && l.inRegion ? " inner" : ""}`}
        fid={`cat/${l.key}`}
        initial={i === startAt}
        label={l.kind === "region" ? `${l.region}, ${l.groups} groups` : l.label}
        style={top}
        onFocus={() => setAt(i)}
        onSelect={() => {
          if (l.kind === "region") setOpen((o) => ({ ...o, [l.region]: !o[l.region] }));
          else p.onSelect(l.category);
        }}
      >
        {l.kind === "category" && <Icon name={l.icon} size={18} class="live-cat-icon" />}
        {l.kind === "region" && <Icon name={l.open ? "chevron-down" : "chevron-right"} size={16} class="live-cat-icon" />}
        {l.kind === "region" ? (
          <>
            <span class="live-cat-region t-badge">{l.region}</span>
            <span class="t-caption live-cat-sub one-line">{`${l.groups} groups`}</span>
          </>
        ) : (
          <span class="t-label one-line live-cat-label">{l.label}</span>
        )}
        {l.count !== null && <span class="t-caption live-cat-count">{count(l.count)}</span>}
      </F>,
    );
  }
  return (
    <div ref={trap} class="panel-root" style="position:absolute;left:0;top:0;right:0;bottom:0;z-index:30" aria-label="Channel groups">
      <div class={`scrim${inside ? " in" : ""}`} onClick={p.onClose} />
      <div class={`live-cats${inside ? " in" : ""}`} style={`left:${p.left}px`}>
        <div class="t-title c-fg live-cats-title">Categories</div>
        <div class="live-cats-box" style={`top:${LIST_TOP}px`}>
          <G kind="col" class="live-cats-rows" style={`height:${lines.length * PITCH}px;transform:translateY(${-Math.max(0, shift)}px)`}>
            {rows}
          </G>
        </div>
      </div>
    </div>
  );
}
