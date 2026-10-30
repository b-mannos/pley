import { useMemo, useState } from "react";
import { ChevronRight, Eye, EyeOff, Search } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useLineupGroups, usePatchLineup, type LineupGroup } from "@/api/livetv";
import { useToast } from "@/components/Toast";
import { Button, EmptyState, PageSpinner, Select, TextInput, Toggle } from "@/components/ui";
import { cn } from "@/lib/cn";
import { pluralize } from "@/lib/format";

type Shown = "all" | "shown" | "hidden";

type Section = { region: string | null; groups: LineupGroup[] };

/** Groups by region in the lineup's order; groups of no region (or a region of one) on their own. */
function sections(groups: LineupGroup[]): Section[] {
  const byRegion = new Map<string, LineupGroup[]>();
  for (const g of groups) {
    if (g.region) byRegion.set(g.region, [...(byRegion.get(g.region) ?? []), g]);
  }
  const out: Section[] = [];
  const placed = new Set<string>();
  for (const g of groups) {
    const region = g.region && (byRegion.get(g.region)?.length ?? 0) > 1 ? g.region : null;
    if (!region) out.push({ region: null, groups: [g] });
    else if (!placed.has(region)) {
      placed.add(region);
      out.push({ region, groups: byRegion.get(region)! });
    }
  }
  return out;
}

const hiddenAll = (gs: LineupGroup[]) => gs.every((g) => g.hidden === g.count);
const shownAll = (gs: LineupGroup[]) => gs.every((g) => g.hidden === 0);

/**
 * Settings → Live TV → Groups: whole groups, and whole regions of them,
 * shown or hidden at once. A provider's hundreds of groups come down to
 * the ones the household watches.
 */
export function Groups() {
  const groups = useLineupGroups();
  const patch = usePatchLineup();
  const toast = useToast();
  const [search, setSearch] = useState("");
  const [shown, setShown] = useState<Shown>("all");
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const all = groups.data;
  const q = search.trim().toLowerCase();
  const listed = useMemo(
    () =>
      sections(
        (all ?? []).filter(
          (g) =>
            (!q || g.name.toLowerCase().includes(q)) &&
            (shown === "all" || (shown === "hidden" ? g.hidden === g.count : g.hidden < g.count)),
        ),
      ),
    [all, q, shown],
  );

  if (groups.isPending) return <PageSpinner />;
  if (groups.isError) {
    return <EmptyState title="Couldn't load the groups" description={errorMessage(groups.error)} action={<Button onClick={() => void groups.refetch()}>Retry</Button>} />;
  }
  if (!all?.length) return <EmptyState title="No groups yet" description="Groups show here once a source has been read." />;

  const setHidden = (names: string[], hidden: boolean, what: string) =>
    patch.mutate(
      { ids: [], groups: names, hidden },
      {
        onSuccess: () => toast.success(`${what} ${hidden ? "hidden" : "shown"}`),
        onError: (e) => toast.error(errorMessage(e)),
      },
    );
  const channels = all.reduce((n, g) => n + g.count, 0);
  const hiddenChannels = all.reduce((n, g) => n + g.hidden, 0);
  const hiddenGroups = all.filter((g) => g.hidden === g.count).length;
  const toggle = (region: string) =>
    setOpen((o) => {
      const next = new Set(o);
      if (next.has(region)) next.delete(region);
      else next.add(region);
      return next;
    });

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold">Groups</h3>
        <p className="text-sm text-muted">
          {pluralize(all.length - hiddenGroups, "group")} shown{hiddenGroups ? `, ${hiddenGroups} hidden` : ""} ·{" "}
          {pluralize(channels - hiddenChannels, "channel")} to watch. Hide the groups nobody here watches: they leave the guide, the
          channel lists and search for everyone.
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        <div className="relative min-w-48 flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-subtle" />
          <TextInput value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Find a group" aria-label="Find a group" className="h-9 pl-9" />
        </div>
        <Select value={shown} onChange={(e) => setShown(e.target.value as Shown)} aria-label="Which">
          <option value="all">Shown and hidden</option>
          <option value="shown">Shown</option>
          <option value="hidden">Hidden</option>
        </Select>
      </div>

      <div className="overflow-hidden rounded-lg border border-border bg-surface">
        {listed.length === 0 ? (
          <p className="p-6 text-center text-sm text-muted">No groups match.</p>
        ) : (
          <ul className="max-h-[min(40rem,68dvh)] divide-y divide-border/60 overflow-y-auto [scrollbar-width:thin]">
            {listed.map((s) =>
              s.region ? (
                <li key={`r:${s.region}`}>
                  <div className="flex h-12 items-center gap-2 px-2 sm:px-3">
                    <button
                      type="button"
                      onClick={() => toggle(s.region!)}
                      aria-expanded={open.has(s.region) || !!q}
                      className="flex min-w-0 flex-1 items-center gap-2.5 rounded-md py-1.5 text-left"
                    >
                      <ChevronRight className={cn("size-4 shrink-0 text-subtle transition-transform", (open.has(s.region) || q) && "rotate-90")} />
                      <span className="min-w-9 rounded bg-surface-3 px-1.5 py-0.5 text-center text-xs font-bold tracking-wide">{s.region}</span>
                      <span className={cn("truncate text-sm", hiddenAll(s.groups) && "text-subtle")}>
                        {pluralize(s.groups.length, "group")} · {s.groups.reduce((n, g) => n + g.count, 0).toLocaleString()} channels
                      </span>
                      {!hiddenAll(s.groups) && !shownAll(s.groups) ? (
                        <span className="hidden shrink-0 text-xs text-subtle sm:inline">some hidden</span>
                      ) : null}
                    </button>
                    <Button
                      size="sm"
                      variant="ghost"
                      icon={hiddenAll(s.groups) ? <Eye className="size-4" /> : <EyeOff className="size-4" />}
                      onClick={() => setHidden(s.groups.map((g) => g.name), !hiddenAll(s.groups), `${s.region}'s groups`)}
                      disabled={patch.isPending}
                    >
                      {hiddenAll(s.groups) ? "Show all" : "Hide all"}
                    </Button>
                  </div>
                  {open.has(s.region) || q ? (
                    <ul className="mb-2 ml-6 border-l border-border pl-2 sm:ml-8">
                      {s.groups.map((g) => (
                        <GroupRow key={g.name} group={g} label={g.label} busy={patch.isPending} onHidden={(h) => setHidden([g.name], h, g.name)} />
                      ))}
                    </ul>
                  ) : null}
                </li>
              ) : (
                <GroupRow key={s.groups[0]!.name} group={s.groups[0]!} label={s.groups[0]!.name} busy={patch.isPending} onHidden={(h) => setHidden([s.groups[0]!.name], h, s.groups[0]!.name || "The group")} padded />
              ),
            )}
          </ul>
        )}
      </div>
    </div>
  );
}

function GroupRow({
  group: g,
  label,
  busy,
  onHidden,
  padded,
}: {
  group: LineupGroup;
  label: string;
  busy: boolean;
  onHidden: (hidden: boolean) => void;
  padded?: boolean;
}) {
  const hidden = g.hidden === g.count;
  return (
    <li className={cn("flex h-11 items-center gap-3 pr-2 sm:pr-3", padded ? "pl-4 sm:pl-5" : "pl-2")}>
      <span className={cn("min-w-0 flex-1 truncate text-sm", hidden && "text-subtle line-through decoration-subtle/40")}>{label || "No group"}</span>
      <span className="shrink-0 text-xs tabular-nums text-subtle">
        {[
          g.hidden > 0 && !hidden ? `${g.hidden} hidden` : null,
          g.offline > 0 ? `${g.offline} offline` : null,
          g.count.toLocaleString(),
        ]
          .filter(Boolean)
          .join(" · ")}
      </span>
      <Toggle checked={!hidden} onChange={(show) => onHidden(!show)} label={`Show ${label || "No group"}`} disabled={busy} />
    </li>
  );
}
