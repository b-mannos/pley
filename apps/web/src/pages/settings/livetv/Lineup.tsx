import { useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { CalendarCheck, CalendarX, Eye, EyeOff, Pencil, Search } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useLineup, useLiveSources, usePatchLineup, type LineupChannel } from "@/api/livetv";
import { Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Button, EmptyState, Field, IconButton, PageSpinner, Select, TextInput } from "@/components/ui";
import { cn } from "@/lib/cn";
import { pluralize } from "@/lib/format";
import { ChannelLogo } from "@/pages/livetv/common";

type Shown = "all" | "shown" | "hidden" | "no-guide";

const ROW = 56;

export function Lineup() {
  const lineup = useLineup();
  const sources = useLiveSources();
  const patch = usePatchLineup();
  const toast = useToast();
  const [search, setSearch] = useState("");
  const [source, setSource] = useState("all");
  const [group, setGroup] = useState("all");
  const [shown, setShown] = useState<Shown>("all");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [editing, setEditing] = useState<LineupChannel | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const all = lineup.data;
  const groups = useMemo(() => [...new Set((all ?? []).filter((c) => source === "all" || String(c.source_id) === source).map((c) => c.group).filter(Boolean))], [all, source]);
  const list = useMemo(() => {
    const q = search.trim().toLowerCase();
    return (all ?? []).filter(
      (c) =>
        (source === "all" || String(c.source_id) === source) &&
        (group === "all" || c.group === group) &&
        (shown === "all" || (shown === "hidden" ? c.hidden : shown === "shown" ? !c.hidden : !c.has_guide)) &&
        (!q || c.name.toLowerCase().includes(q) || c.source_name.toLowerCase().includes(q) || (c.number ?? "").startsWith(q)),
    );
  }, [all, search, source, group, shown]);
  // TanStack Virtual: the compiler leaves this component as it is.
  // eslint-disable-next-line react-hooks/incompatible-library
  const rows = useVirtualizer({ count: list.length, getScrollElement: () => scrollRef.current, estimateSize: () => ROW, overscan: 10 });

  if (lineup.isPending) return <PageSpinner />;
  if (lineup.isError) {
    return <EmptyState title="Couldn't load the channels" description={errorMessage(lineup.error)} action={<Button onClick={() => void lineup.refetch()}>Retry</Button>} />;
  }
  if (!all?.length) {
    return <EmptyState title="No channels yet" description="Channels show here once a source has been read." />;
  }

  const allSelected = list.length > 0 && list.every((c) => selected.has(c.id));
  const chosen = list.filter((c) => selected.has(c.id));
  const setHidden = (ids: number[], hidden: boolean) =>
    patch.mutate(
      { ids, hidden },
      {
        onSuccess: () => {
          if (ids.length > 1) toast.success(`${pluralize(ids.length, "channel")} ${hidden ? "hidden" : "shown"}`);
          setSelected(new Set());
        },
        onError: (e) => toast.error(errorMessage(e)),
      },
    );
  const hiddenCount = all.filter((c) => c.hidden).length;

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold">Channels</h3>
        <p className="text-sm text-muted">
          {pluralize(all.length - hiddenCount, "channel")} shown{hiddenCount ? `, ${hiddenCount} hidden` : ""}. Hidden channels are nobody's to watch; names
          and numbers set here stay when the source is read again.
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        <div className="relative min-w-48 flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-subtle" />
          <TextInput value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Find a channel" aria-label="Find a channel" className="h-9 pl-9" />
        </div>
        {(sources.data?.length ?? 0) > 1 ? (
          <Select value={source} onChange={(e) => (setSource(e.target.value), setGroup("all"))} aria-label="Source">
            <option value="all">Every source</option>
            {sources.data?.map((s) => (
              <option key={s.id} value={String(s.id)}>
                {s.name}
              </option>
            ))}
          </Select>
        ) : null}
        {groups.length > 1 ? (
          <Select value={group} onChange={(e) => setGroup(e.target.value)} aria-label="Group" className="max-w-52">
            <option value="all">Every group</option>
            {groups.map((g) => (
              <option key={g} value={g}>
                {g}
              </option>
            ))}
          </Select>
        ) : null}
        <Select value={shown} onChange={(e) => setShown(e.target.value as Shown)} aria-label="Which">
          <option value="all">Shown and hidden</option>
          <option value="shown">Shown</option>
          <option value="hidden">Hidden</option>
          <option value="no-guide">Without a guide</option>
        </Select>
      </div>

      <div className="overflow-hidden rounded-lg border border-border bg-surface">
        <div className="flex h-11 items-center gap-3 border-b border-border bg-surface-2/60 px-3 text-xs font-medium text-subtle">
          <input
            type="checkbox"
            aria-label="Select every channel listed"
            checked={allSelected}
            onChange={() => setSelected(allSelected ? new Set() : new Set(list.map((c) => c.id)))}
            className="size-4 accent-[var(--color-accent)]"
          />
          {chosen.length ? (
            <div className="flex flex-1 items-center gap-2 text-fg animate-fade-in">
              <span className="text-sm">{chosen.length} selected</span>
              <Button size="sm" variant="ghost" icon={<EyeOff className="size-4" />} onClick={() => setHidden(chosen.map((c) => c.id), true)} loading={patch.isPending}>
                Hide
              </Button>
              <Button size="sm" variant="ghost" icon={<Eye className="size-4" />} onClick={() => setHidden(chosen.map((c) => c.id), false)} loading={patch.isPending}>
                Show
              </Button>
            </div>
          ) : (
            <span className="flex-1">{pluralize(list.length, "channel")}</span>
          )}
        </div>
        {list.length === 0 ? (
          <p className="p-6 text-center text-sm text-muted">No channels match.</p>
        ) : (
          <div ref={scrollRef} className="h-[min(36rem,65dvh)] overflow-y-auto [scrollbar-width:thin]">
            <ul className="relative" style={{ height: rows.getTotalSize() }}>
              {rows.getVirtualItems().map((row) => {
                const c = list[row.index]!;
                return (
                  <li
                    key={c.id}
                    className={cn("absolute inset-x-0 flex items-center gap-3 border-b border-border/60 px-3", c.hidden && "opacity-50")}
                    style={{ top: row.start, height: ROW }}
                  >
                    <input
                      type="checkbox"
                      aria-label={`Select ${c.name}`}
                      checked={selected.has(c.id)}
                      onChange={() =>
                        setSelected((s) => {
                          const next = new Set(s);
                          if (next.has(c.id)) next.delete(c.id);
                          else next.add(c.id);
                          return next;
                        })
                      }
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
                    <span
                      className={cn("hidden items-center gap-1 text-xs sm:flex", c.has_guide ? "text-success" : "text-subtle")}
                      title={c.has_guide ? `Guide: ${c.guide_id}` : "No guide"}
                    >
                      {c.has_guide ? <CalendarCheck className="size-3.5" /> : <CalendarX className="size-3.5" />}
                      {c.has_guide ? "Guide" : "No guide"}
                    </span>
                    <IconButton label={c.hidden ? `Show ${c.name}` : `Hide ${c.name}`} size="sm" onClick={() => setHidden([c.id], !c.hidden)}>
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
