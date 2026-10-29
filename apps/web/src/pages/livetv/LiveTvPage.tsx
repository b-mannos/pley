import { useMemo, useState } from "react";
import { Link } from "react-router";
import { CalendarDays, LayoutGrid, RefreshCw, Search, Settings2, Star, Tv2, X } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useChannels, useLiveSummary, type Channel, type Program } from "@/api/livetv";
import { useAuth } from "@/auth";
import { Button, buttonClass, EmptyState, Skeleton, TextInput } from "@/components/ui";
import { cn } from "@/lib/cn";
import { useDocumentTitle, useLocalStorage } from "@/lib/hooks";
import { ChannelGrid, ChannelRow } from "./ChannelGrid";
import { filterKey, parseFilter, useFilteredChannels, useGroups, useNow, type ChannelFilter } from "./common";
import { GuideGrid } from "./GuideGrid";
import { ProgramModal } from "./ProgramModal";

type View = "guide" | "channels";

export function LiveTvPage() {
  useDocumentTitle("Live TV");
  const user = useAuth();
  const summary = useLiveSummary();
  const channels = useChannels();
  const now = useNow();
  const [view, setView] = useLocalStorage<View>("pley.live.view", "guide");
  const [filterValue, setFilterValue] = useLocalStorage("pley.live.filter", "all");
  const filter = parseFilter(filterValue);
  const [search, setSearch] = useState("");
  const [details, setDetails] = useState<{ channel: Channel; program: Program } | null>(null);
  const groups = useGroups(channels.data);
  const favorites = channels.data?.filter((c) => c.favorite).length ?? 0;
  // A filter whose group went away shows everything.
  const effective: ChannelFilter =
    (filter.kind === "group" && !groups.some((g) => g.name === filter.group)) || (filter.kind === "favorites" && favorites === 0)
      ? { kind: "all" }
      : filter;
  const shown = useFilteredChannels(channels.data, effective, search);
  const recent = useMemo(() => {
    const byId = new Map((channels.data ?? []).map((c) => [c.id, c]));
    return (summary.data?.recent ?? []).map((id) => byId.get(id)).filter((c): c is Channel => !!c).slice(0, 6);
  }, [channels.data, summary.data?.recent]);
  // Details follow the channel as it refreshes (a favourite set, say).
  const detailChannel = details ? (channels.data?.find((c) => c.id === details.channel.id) ?? details.channel) : null;

  if (summary.data && !summary.data.allowed) {
    return (
      <Page>
        <EmptyState icon={<Tv2 />} title="Live TV isn't open to this account" description="An administrator can open it in Settings → Live TV." />
      </Page>
    );
  }
  if (channels.isError) {
    return (
      <Page>
        <EmptyState
          icon={<Tv2 />}
          title="Live TV didn't load"
          description={errorMessage(channels.error)}
          action={
            <Button icon={<RefreshCw className="size-4" />} onClick={() => void channels.refetch()}>
              Try again
            </Button>
          }
        />
      </Page>
    );
  }
  if (channels.data && channels.data.length === 0) {
    return (
      <Page>
        <EmptyState
          icon={<Tv2 />}
          title={user.is_admin ? "Watch TV on Pley" : "No channels yet"}
          description={
            user.is_admin
              ? "Add your IPTV provider's playlist or Xtream Codes account: Pley reads its channels and guide, and everyone watches through this server."
              : "Channels appear here once an administrator adds an IPTV provider."
          }
          action={
            user.is_admin ? (
              <Link to="/settings/livetv" className={buttonClass("primary")}>
                <Settings2 className="size-4" /> Set up Live TV
              </Link>
            ) : undefined
          }
        />
      </Page>
    );
  }

  return (
    <Page
      actions={
        <div className="flex items-center gap-2">
          <div role="tablist" aria-label="View" className="flex rounded-lg border border-border bg-surface p-0.5">
            {(
              [
                ["guide", "Guide", CalendarDays],
                ["channels", "Channels", LayoutGrid],
              ] as const
            ).map(([key, label, Icon]) => (
              <button
                key={key}
                role="tab"
                aria-selected={view === key}
                onClick={() => setView(key)}
                className={cn(
                  "flex h-8 items-center gap-1.5 rounded-md px-3 text-sm font-medium transition-colors",
                  view === key ? "bg-surface-3 text-fg shadow-sm" : "text-muted hover:text-fg",
                )}
              >
                <Icon className="size-4" />
                <span className="hidden sm:inline">{label}</span>
              </button>
            ))}
          </div>
          {user.is_admin ? (
            <Link to="/settings/livetv" className={buttonClass("ghost", "sm", "size-9 px-0")} aria-label="Live TV settings" title="Live TV settings">
              <Settings2 className="size-4" />
            </Link>
          ) : null}
        </div>
      }
    >
      <div className="mb-4 flex flex-col gap-3 lg:flex-row lg:items-center">
        <div className="relative lg:w-72">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-subtle" />
          <TextInput
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Find a channel or what's on"
            aria-label="Find a channel"
            className="pl-9 pr-9"
          />
          {search ? (
            <button
              type="button"
              onClick={() => setSearch("")}
              aria-label="Clear"
              className="absolute right-2 top-1/2 flex size-6 -translate-y-1/2 items-center justify-center rounded-full text-subtle hover:bg-white/10 hover:text-fg"
            >
              <X className="size-3.5" />
            </button>
          ) : null}
        </div>
        <div className="scrollbar-none -mx-4 flex min-w-0 flex-1 gap-1.5 overflow-x-auto px-4 lg:mx-0 lg:px-0" role="group" aria-label="Show">
          <Chip active={effective.kind === "all"} onClick={() => setFilterValue("all")}>
            All <Count n={channels.data?.length} />
          </Chip>
          {favorites > 0 ? (
            <Chip active={effective.kind === "favorites"} onClick={() => setFilterValue("favorites")}>
              <Star className="size-3.5" /> Favourites <Count n={favorites} />
            </Chip>
          ) : null}
          {groups.length > 1
            ? groups.map((g) => (
                <Chip
                  key={g.name}
                  active={effective.kind === "group" && effective.group === g.name}
                  onClick={() => setFilterValue(filterKey({ kind: "group", group: g.name }))}
                >
                  {g.name} <Count n={g.count} />
                </Chip>
              ))
            : null}
        </div>
      </div>

      {!channels.data ? (
        <Skeleton className="h-[calc(100dvh-13.5rem)] min-h-[22rem] w-full rounded-lg" />
      ) : shown.length === 0 ? (
        <EmptyState
          icon={<Search />}
          title="No channels match"
          description={search ? `Nothing called “${search}” here.` : "No channels in this group."}
          action={
            <Button
              onClick={() => {
                setSearch("");
                setFilterValue("all");
              }}
            >
              Show every channel
            </Button>
          }
        />
      ) : view === "guide" ? (
        <GuideGrid channels={shown} now={now} onDetails={(channel, program) => setDetails({ channel, program })} />
      ) : (
        <>
          {recent.length > 0 && !search && effective.kind === "all" ? (
            <section className="mb-8">
              <h2 className="mb-3 text-sm font-semibold uppercase tracking-wider text-subtle">Recently watched</h2>
              <ChannelRow channels={recent} now={now} onDetails={(channel, program) => setDetails({ channel, program })} />
            </section>
          ) : null}
          <ChannelGrid channels={shown} now={now} onDetails={(channel, program) => setDetails({ channel, program })} />
        </>
      )}
      <ProgramModal channel={detailChannel} program={details?.program ?? null} onClose={() => setDetails(null)} />
    </Page>
  );
}

function Page({ children, actions }: { children: React.ReactNode; actions?: React.ReactNode }) {
  return (
    <div className="px-4 pb-12 pt-6 sm:px-8">
      <div className="mb-5 flex items-center gap-3">
        <div className="flex size-10 items-center justify-center rounded-lg bg-surface-2 text-accent">
          <Tv2 className="size-5" />
        </div>
        <h1 className="flex-1 text-2xl font-bold tracking-tight">Live TV</h1>
        {actions}
      </div>
      {children}
    </div>
  );
}

function Chip({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "flex h-8 shrink-0 items-center gap-1.5 rounded-full border px-3 text-sm font-medium transition-colors",
        active ? "border-accent/50 bg-accent/15 text-accent" : "border-border text-muted hover:border-border-strong hover:text-fg",
      )}
    >
      {children}
    </button>
  );
}

function Count({ n }: { n: number | undefined }) {
  return n === undefined ? null : <span className="text-xs tabular-nums opacity-70">{n}</span>;
}
