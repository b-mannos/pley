import { useEffect, useMemo, useRef, useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { CalendarCheck, CalendarX, Eye, EyeOff, Pencil, Search } from "lucide-react";
import { errorMessage } from "@/api/client";
import {
  CHANNEL_PAGE,
  liveQueries,
  useLineupGroups,
  useLiveSources,
  usePatchLineup,
  type LineupChannel,
  type LineupFilter,
  type LineupState,
} from "@/api/livetv";
import { Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Button, EmptyState, Field, IconButton, PageSpinner, Select, TextInput } from "@/components/ui";
import { cn } from "@/lib/cn";
import { pluralize } from "@/lib/format";
import { ChannelLogo } from "@/pages/livetv/common";

const ROW = 56;

/** What's chosen: some channels by id, or every one the filter keeps. */
type Chosen = { all: false; ids: ReadonlySet<number> } | { all: true };

/**
 * Settings → Live TV → Channels: the lineup, hidden channels too, filtered
 * on the server and fetched a page at a time as it scrolls, so a lineup of
 * tens of thousands opens at once.
 */
export function Lineup() {
  const sources = useLiveSources();
  const groups = useLineupGroups();
  const patch = usePatchLineup();
  const toast = useToast();
  const [search, setSearch] = useState("");
  const [source, setSource] = useState("all");
  const [group, setGroup] = useState("all");
  const [shown, setShown] = useState<LineupState>("all");
  const [chosen, setChosen] = useState<Chosen>({ all: false, ids: new Set() });
  const [editing, setEditing] = useState<LineupChannel | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const filter = useMemo<LineupFilter>(
    () => ({
      source_id: source === "all" ? undefined : Number(source),
      group: group === "all" ? undefined : group,
      state: shown,
      q: search.trim() || undefined,
    }),
    [source, group, shown, search],
  );
  // Another filter, another choice.
  useEffect(() => setChosen({ all: false, ids: new Set() }), [filter]);
  const [range, setRange] = useState({ from: 0, to: 20 });
  // The first page says how many there are; the others come as they're scrolled to.
  const everything = useQuery(liveQueries.lineupPage({}, 0));
  const first = useQuery(liveQueries.lineupPage(filter, 0));
  const total = first.data?.total ?? 0;
  const lastPage = Math.max(0, Math.ceil(total / CHANNEL_PAGE) - 1);
  const pages: number[] = [];
  for (let p = Math.max(1, Math.floor(range.from / CHANNEL_PAGE)); p <= Math.min(lastPage, Math.floor(range.to / CHANNEL_PAGE)); p++) pages.push(p);
  const rest = useQueries({ queries: pages.map((p) => liveQueries.lineupPage(filter, p)) });
  const at = (i: number) => {
    const page = Math.floor(i / CHANNEL_PAGE);
    const data = page === 0 ? first.data : rest[pages.indexOf(page)]?.data;
    return data?.channels[i % CHANNEL_PAGE];
  };

  // TanStack Virtual: the compiler leaves this component as it is.
  // eslint-disable-next-line react-hooks/incompatible-library
  const rows = useVirtualizer({ count: total, getScrollElement: () => scrollRef.current, estimateSize: () => ROW, overscan: 10 });
  const items = rows.getVirtualItems();
  const firstRow = items[0]?.index ?? 0;
  const lastRow = items[items.length - 1]?.index ?? 20;
  useEffect(() => {
    setRange((r) => (r.from === firstRow && r.to === lastRow ? r : { from: firstRow, to: lastRow }));
  }, [firstRow, lastRow]);
  useEffect(() => scrollRef.current?.scrollTo({ top: 0 }), [filter]);

  const groupNames = useMemo(() => (groups.data ?? []).map((g) => g.name).filter(Boolean), [groups.data]);

  if (everything.isPending) return <PageSpinner />;
  if (everything.isError) {
    return <EmptyState title="Couldn't load the channels" description={errorMessage(everything.error)} action={<Button onClick={() => void everything.refetch()}>Retry</Button>} />;
  }
  if (!everything.data?.total) {
    return <EmptyState title="No channels yet" description="Channels show here once a source has been read." />;
  }

  const chosenCount = chosen.all ? total : chosen.ids.size;
  const setHidden = (hidden: boolean, ids: number[] | null) =>
    patch.mutate(
      ids ? { ids, hidden } : { ids: [], filter, hidden },
      {
        onSuccess: () => {
          const n = ids ? ids.length : total;
          if (n > 1) toast.success(`${pluralize(n, "channel")} ${hidden ? "hidden" : "shown"}`);
          setChosen({ all: false, ids: new Set() });
        },
        onError: (e) => toast.error(errorMessage(e)),
      },
    );
  const toggle = (id: number) =>
    setChosen((c) => {
      const ids = new Set(c.all ? [] : c.ids);
      if (ids.has(id)) ids.delete(id);
      else ids.add(id);
      return { all: false, ids };
    });
  const isChosen = (id: number) => chosen.all || chosen.ids.has(id);
  const all = everything.data;

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold">Channels</h3>
        <p className="text-sm text-muted">
          {pluralize(all.total - all.hidden, "channel")} shown{all.hidden ? `, ${all.hidden.toLocaleString()} hidden` : ""}. Hidden channels are nobody's to
          watch; names and numbers set here stay when the source is read again. Whole groups are hidden in Groups.
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        <div className="relative min-w-48 flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-subtle" />
          <TextInput value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Find a channel" aria-label="Find a channel" className="h-9 pl-9" />
        </div>
        {(sources.data?.length ?? 0) > 1 ? (
          <Select value={source} onChange={(e) => setSource(e.target.value)} aria-label="Source">
            <option value="all">Every source</option>
            {sources.data?.map((s) => (
              <option key={s.id} value={String(s.id)}>
                {s.name}
              </option>
            ))}
          </Select>
        ) : null}
        {groupNames.length > 1 ? (
          <Select value={group} onChange={(e) => setGroup(e.target.value)} aria-label="Group" className="max-w-52">
            <option value="all">Every group</option>
            {groupNames.map((g) => (
              <option key={g} value={g}>
                {g}
              </option>
            ))}
          </Select>
        ) : null}
        <Select value={shown} onChange={(e) => setShown(e.target.value as LineupState)} aria-label="Which">
          <option value="all">Shown and hidden</option>
          <option value="shown">Shown</option>
          <option value="hidden">Hidden</option>
          <option value="no-guide">Without a guide</option>
          <option value="offline">Offline</option>
        </Select>
      </div>

      <div className="overflow-hidden rounded-lg border border-border bg-surface">
        <div className="flex h-11 items-center gap-3 border-b border-border bg-surface-2/60 px-3 text-xs font-medium text-subtle">
          <input
            type="checkbox"
            aria-label="Choose every channel listed"
            checked={total > 0 && chosen.all}
            onChange={() => setChosen(chosen.all ? { all: false, ids: new Set() } : { all: true })}
            className="size-4 accent-[var(--color-accent)]"
          />
          {chosenCount ? (
            <div className="flex flex-1 items-center gap-2 text-fg animate-fade-in">
              <span className="text-sm">{chosenCount.toLocaleString()} chosen</span>
              <Button size="sm" variant="ghost" icon={<EyeOff className="size-4" />} onClick={() => setHidden(true, chosen.all ? null : [...chosen.ids])} loading={patch.isPending}>
                Hide
              </Button>
              <Button size="sm" variant="ghost" icon={<Eye className="size-4" />} onClick={() => setHidden(false, chosen.all ? null : [...chosen.ids])} loading={patch.isPending}>
                Show
              </Button>
            </div>
          ) : (
            <span className="flex-1">{first.data ? pluralize(total, "channel") : "…"}</span>
          )}
        </div>
        {first.data && total === 0 ? (
          <p className="p-6 text-center text-sm text-muted">No channels match.</p>
        ) : (
          <div ref={scrollRef} className="h-[min(36rem,65dvh)] overflow-y-auto [scrollbar-width:thin]">
            <ul className="relative" style={{ height: rows.getTotalSize() }}>
              {items.map((row) => {
                const c = at(row.index);
                if (!c) {
                  return (
                    <li key={`r${row.index}`} aria-hidden className="absolute inset-x-0 flex items-center gap-3 border-b border-border/60 px-3" style={{ top: row.start, height: ROW }}>
                      <span className="size-4" />
                      <span className="skeleton h-8 w-12 rounded" />
                      <span className="skeleton h-3.5 w-1/3 rounded" />
                    </li>
                  );
                }
                return (
                  <li
                    key={c.id}
                    className={cn("absolute inset-x-0 flex items-center gap-3 border-b border-border/60 px-3", c.hidden && "opacity-50")}
                    style={{ top: row.start, height: ROW }}
                  >
                    <input
                      type="checkbox"
                      aria-label={`Choose ${c.name}`}
                      checked={isChosen(c.id)}
                      onChange={() => toggle(c.id)}
                      className="size-4 accent-[var(--color-accent)]"
                    />
                    <span className="w-10 shrink-0 text-right text-xs tabular-nums text-subtle">{c.number ?? ""}</span>
                    <ChannelLogo channel={c} width={56} showName={false} className="h-8 w-12 shrink-0 rounded bg-surface-3" />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-medium">{c.name}</div>
                      <div className="truncate text-xs text-subtle">
                        {[c.group, c.custom_name ? `“${c.source_name}” at the source` : null].filter(Boolean).join(" · ")}
                      </div>
                    </div>
                    {c.offline ? (
                      <span className="hidden rounded bg-danger/15 px-1.5 py-0.5 text-[11px] font-medium text-danger sm:inline" title="It didn't play when last tried">
                        Offline
                      </span>
                    ) : null}
                    <span
                      className={cn("hidden items-center gap-1 text-xs sm:flex", c.has_guide ? "text-success" : "text-subtle")}
                      title={c.has_guide ? `Guide: ${c.guide_id}` : "No guide"}
                    >
                      {c.has_guide ? <CalendarCheck className="size-3.5" /> : <CalendarX className="size-3.5" />}
                      {c.has_guide ? "Guide" : "No guide"}
                    </span>
                    <IconButton label={c.hidden ? `Show ${c.name}` : `Hide ${c.name}`} size="sm" onClick={() => setHidden(!c.hidden, [c.id])}>
                      {c.hidden ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
                    </IconButton>
                    <IconButton label={`Edit ${c.name}`} size="sm" onClick={() => setEditing(c)}>
                      <Pencil className="size-4" />
                    </IconButton>
                  </li>
                );
              })}
            </ul>
          </div>
        )}
      </div>
      {editing ? <ChannelEditor channel={editing} onClose={() => setEditing(null)} /> : null}
    </div>
  );
}

function ChannelEditor({ channel, onClose }: { channel: LineupChannel; onClose: () => void }) {
  const patch = usePatchLineup();
  const toast = useToast();
  const [name, setName] = useState(channel.custom_name ?? "");
  const [number, setNumber] = useState(channel.custom_number ?? "");
  const [guide, setGuide] = useState(channel.custom_guide_id ?? "");
  const save = () =>
    patch.mutate(
      { ids: [channel.id], name, number, guide_id: guide },
      {
        onSuccess: () => {
          toast.success("Channel saved");
          onClose();
        },
        onError: (e) => toast.error(errorMessage(e)),
      },
    );
  return (
    <Modal
      open
      onClose={onClose}
      locked={patch.isPending}
      title={
        <span className="flex items-center gap-3">
          <ChannelLogo channel={channel} width={48} showName={false} className="h-8 w-12 rounded bg-surface-3" />
          {channel.source_name}
        </span>
      }
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={patch.isPending}>
            Cancel
          </Button>
          <Button variant="primary" onClick={save} loading={patch.isPending}>
            Save
          </Button>
        </>
      }
    >
      <form
        className="space-y-4"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <Field label="Name" htmlFor="ch-name" hint="Empty: the source's.">
          <TextInput id="ch-name" value={name} onChange={(e) => setName(e.target.value)} placeholder={channel.source_name} />
        </Field>
        <Field label="Number" htmlFor="ch-number" hint={channel.source_number ? `Empty: the source's, ${channel.source_number}.` : "Channels go by number, then as their source lists them."}>
          <TextInput id="ch-number" value={number} onChange={(e) => setNumber(e.target.value)} placeholder={channel.source_number ?? ""} inputMode="decimal" />
        </Field>
        <Field
          label="Guide channel"
          htmlFor="ch-guide"
          hint={
            channel.source_guide_id
              ? `Its id in the guide (XMLTV). Empty: the source's, ${channel.source_guide_id}.`
              : channel.matched_guide_id
                ? `Its id in the guide (XMLTV). Empty: ${channel.matched_guide_id}, found by its name.`
                : "Its id in the guide (XMLTV): the source names none, and none goes by its name."
          }
        >
          <TextInput
            id="ch-guide"
            value={guide}
            onChange={(e) => setGuide(e.target.value)}
            placeholder={channel.source_guide_id || channel.matched_guide_id || "bbc1.uk"}
            spellCheck={false}
            className="font-mono"
          />
        </Field>
        <p className="text-xs text-subtle">A new guide channel shows its programmes once the guide is read again.</p>
        <button type="submit" hidden />
      </form>
    </Modal>
  );
}
