import { useMemo, useState } from "react";
import { ChevronRight, History, LayoutGrid, Search, Star, X } from "lucide-react";
import { categoryKey, type LiveCategory, type LiveGroup, type LiveGroups } from "@/api/livetv";
import { Skeleton } from "@/components/ui";
import { cn } from "@/lib/cn";

type Entry = { kind: "group"; group: LiveGroup } | { kind: "region"; region: string; groups: LiveGroup[]; count: number };

/**
 * Groups in the provider's order, those of a region ("UK| Sport", "UK|
 * News"…) gathered under it, so hundreds of groups read as a few dozen.
 * A region with one group stays a group.
 */
export function groupTree(groups: LiveGroup[]): Entry[] {
  const byRegion = new Map<string, LiveGroup[]>();
  for (const g of groups) {
    if (g.region) byRegion.set(g.region, [...(byRegion.get(g.region) ?? []), g]);
  }
  const entries: Entry[] = [];
  const placed = new Set<string>();
  for (const g of groups) {
    const region = g.region ? byRegion.get(g.region)! : null;
    if (!region || region.length < 2) {
      entries.push({ kind: "group", group: g });
    } else if (!placed.has(g.region!)) {
      placed.add(g.region!);
      entries.push({ kind: "region", region: g.region!, groups: region, count: region.reduce((n, x) => n + x.count, 0) });
    }
  }
  return entries;
}

/** What a category is called: "Favourites", "UK · Sport". */
export function categoryName(c: LiveCategory, groups: LiveGroup[] | undefined): string {
  switch (c.kind) {
    case "all":
      return "All channels";
    case "favorites":
      return "Favourites";
    case "recent":
      return "Recently watched";
    case "group": {
      const g = groups?.find((x) => x.name === c.group);
      return g?.region ? `${g.region} · ${g.label}` : (g?.label ?? c.group);
    }
  }
}

const count = (n: number) => n.toLocaleString();

/** The categories down the side of Live TV: the marked ones, all, then every group. */
export function CategoryRail({
  groups,
  selected,
  onSelect,
  className,
}: {
  groups: LiveGroups | undefined;
  selected: LiveCategory;
  onSelect: (c: LiveCategory) => void;
  className?: string;
}) {
  const [search, setSearch] = useState("");
  const tree = useMemo(() => groupTree(groups?.groups ?? []), [groups]);
  const selectedRegion =
    selected.kind === "group" ? (groups?.groups.find((g) => g.name === selected.group)?.region ?? null) : null;
  const [open, setOpen] = useState<ReadonlySet<string>>(() => new Set(selectedRegion ? [selectedRegion] : []));
  const q = search.trim().toLowerCase();
  const found = useMemo(
    () => (q ? (groups?.groups ?? []).filter((g) => g.name.toLowerCase().includes(q)) : []),
    [groups, q],
  );
  const key = categoryKey(selected);
  const toggle = (region: string) =>
    setOpen((o) => {
      const next = new Set(o);
      if (next.has(region)) next.delete(region);
      else next.add(region);
      return next;
    });

  return (
    <nav aria-label="Channel groups" className={cn("flex min-h-0 flex-col", className)}>
      <div className="space-y-0.5 p-2">
        <Item active={key === "favorites"} onClick={() => onSelect({ kind: "favorites" })} icon={<Star className="size-4" />} count={groups?.favorites}>
          Favourites
        </Item>
        <Item active={key === "recent"} onClick={() => onSelect({ kind: "recent" })} icon={<History className="size-4" />} count={groups?.recent}>
          Recently watched
        </Item>
        <Item active={key === "all"} onClick={() => onSelect({ kind: "all" })} icon={<LayoutGrid className="size-4" />} count={groups?.total}>
          All channels
        </Item>
      </div>
      {!groups ? (
        <div className="space-y-2 p-3">
          {Array.from({ length: 8 }, (_, i) => (
            <Skeleton key={i} className="h-7 w-full rounded-md" />
          ))}
        </div>
      ) : groups.groups.length > 1 ? (
        <>
          <div className="flex items-center justify-between border-t border-border px-4 pb-1 pt-3">
            <span className="text-[11px] font-semibold uppercase tracking-wider text-subtle">Groups</span>
            <span className="text-[11px] tabular-nums text-subtle">{count(groups.groups.length)}</span>
          </div>
          {groups.groups.length > 12 ? (
            <div className="relative px-2 pb-2 pt-1">
              <Search className="pointer-events-none absolute left-4.5 top-1/2 size-3.5 -translate-y-1/2 text-subtle" />
              <input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Find a group"
                aria-label="Find a group"
                className="h-8 w-full rounded-md border border-border bg-surface-2 pl-8 pr-8 text-sm placeholder:text-subtle focus:border-accent focus:outline-none"
              />
              {search ? (
                <button
                  type="button"
                  onClick={() => setSearch("")}
                  aria-label="Clear"
                  className="absolute right-3.5 top-1/2 flex size-5 -translate-y-1/2 items-center justify-center rounded-full text-subtle hover:text-fg"
                >
                  <X className="size-3" />
                </button>
              ) : null}
            </div>
          ) : null}
          <div className="min-h-0 flex-1 space-y-0.5 overflow-y-auto overscroll-contain px-2 pb-3 [scrollbar-width:thin]">
            {q ? (
              found.length ? (
                found.map((g) => (
                  <Item key={g.name} active={key === `group:${g.name}`} onClick={() => onSelect({ kind: "group", group: g.name })} count={g.count}>
                    {g.region ? <span className="mr-1.5 text-subtle">{g.region}</span> : null}
                    {g.label}
                  </Item>
                ))
              ) : (
                <p className="px-3 py-4 text-sm text-subtle">No group called “{search.trim()}”.</p>
              )
            ) : (
              tree.map((e) =>
                e.kind === "group" ? (
                  <Item
                    key={e.group.name}
                    active={key === `group:${e.group.name}`}
                    onClick={() => onSelect({ kind: "group", group: e.group.name })}
                    count={e.group.count}
                  >
                    {e.group.name}
                  </Item>
                ) : (
                  <Region
                    key={e.region}
                    entry={e}
                    open={open.has(e.region) || selectedRegion === e.region}
                    onToggle={() => toggle(e.region)}
                    selectedKey={key}
                    onSelect={onSelect}
                  />
                ),
              )
            )}
          </div>
        </>
      ) : null}
    </nav>
  );
}

function Region({
  entry,
  open,
  onToggle,
  selectedKey,
  onSelect,
}: {
  entry: Extract<Entry, { kind: "region" }>;
  open: boolean;
  onToggle: () => void;
  selectedKey: string;
  onSelect: (c: LiveCategory) => void;
}) {
  const holdsSelected = entry.groups.some((g) => selectedKey === `group:${g.name}`);
  return (
    <div>
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={open}
        className={cn(
          "flex h-9 w-full items-center gap-2 rounded-md px-2 text-left text-sm transition-colors hover:bg-surface-2",
          holdsSelected && !open ? "text-accent" : "text-fg",
        )}
      >
        <ChevronRight className={cn("size-3.5 shrink-0 text-subtle transition-transform duration-200", open && "rotate-90")} />
        <span className="min-w-8 rounded bg-surface-3 px-1.5 py-0.5 text-center text-[11px] font-bold tracking-wide">{entry.region}</span>
        <span className="flex-1 truncate text-xs text-subtle">
          {entry.groups.length} groups
        </span>
        <span className="text-xs tabular-nums text-subtle">{count(entry.count)}</span>
      </button>
      {open ? (
        <div className="mb-1 ml-[1.15rem] space-y-0.5 border-l border-border pl-2">
          {entry.groups.map((g) => (
            <Item key={g.name} active={selectedKey === `group:${g.name}`} onClick={() => onSelect({ kind: "group", group: g.name })} count={g.count}>
              {g.label}
            </Item>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function Item({
  active,
  onClick,
  icon,
  count: n,
  children,
}: {
  active: boolean;
  onClick: () => void;
  icon?: React.ReactNode;
  count?: number;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "true" : undefined}
      className={cn(
        "flex min-h-8 w-full items-center gap-2.5 rounded-md px-2.5 py-1 text-left text-sm transition-colors",
        active ? "bg-accent/15 font-medium text-accent" : "text-muted hover:bg-surface-2 hover:text-fg",
      )}
    >
      {icon ? <span className="shrink-0">{icon}</span> : null}
      <span className="min-w-0 flex-1 truncate">{children}</span>
      {n !== undefined ? <span className={cn("shrink-0 text-xs tabular-nums", active ? "text-accent/80" : "text-subtle")}>{count(n)}</span> : null}
    </button>
  );
}
