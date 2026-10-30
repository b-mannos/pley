import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import type { LiveChannel, LiveProgram } from "../../api/types";
import {
  airing,
  channelByNumber,
  effectiveFilter,
  episodeText,
  filterChannels,
  filterKey,
  formatGuideDay,
  formatSpan,
  formatTime,
  groupsOf,
  nowAndNext,
  parseFilter,
  programProgress,
  recentChannels,
  timeLeft,
  type ChannelFilter,
} from "../../live/data";
import { useUi } from "../context";
import { Chip, PanelRow, Tab } from "../kit/buttons";
import { F, G } from "../kit/focusable";
import { Icon } from "../kit/icons";
import { PanelTitle, SidePanel } from "../kit/panel";
import { SectionPage, type Section } from "../kit/row";
import type { Entry } from "../shell/navigator";
import { ColumnMessage, failedTitle, PageBadge, problem, useApp, useLoad } from "./common";
import { LiveGuide } from "./live-guide";
import { ChannelLogo, NumberBox, ProgramMarks, ProgressLine, useNow, useNumberEntry } from "./live-kit";

/** The view and the filter last shown, kept on the TV (the web keeps them in the browser). */
const VIEW_KEY = "pley.live.view";
const FILTER_KEY = "pley.live.filter";
/** What's on changes as time passes: the channels are asked for again this often while the page shows. */
const REFRESH_MS = 60_000;
/** The channels watched last, at most. */
const RECENT_MAX = 6;

type View = "guide" | "channels";
type Panel = null | { kind: "group" } | { kind: "details"; channelId: number; program: LiveProgram | null; opener: string | null };

/**
 * Live TV (the web's LiveTvPage): the guide, channels down and time across,
 * or the channels as cards with what's on now and next and the ones watched
 * last; All, Favourites and the provider's groups; a programme's details;
 * a channel's number typed on the remote jumps to it. Sources and the
 * lineup are set up in Pley on the web, as the TV apps' other settings
 * for administrators are.
 */
export function LiveTvScreen(p: { entry: Entry }) {
  const app = useApp();
  const ui = useUi();
  const entry = p.entry;
  const summary = app.live.value;
  const loaded = useLoad((c) => ui.api.liveChannels(c), [app.liveChanges.value], { initial: app.liveChannels });
  const reload = useRef(loaded.reload);
  reload.current = loaded.reload;
  useEffect(() => {
    // The channels watched last, as the page opens (a channel was just watched, maybe).
    void app.loadLive();
    const t = setInterval(() => reload.current(), REFRESH_MS);
    return () => clearInterval(t);
  }, []);
  useEffect(() => {
    if (loaded.data) app.liveChannels = loaded.data;
  }, [loaded.data]);

  // Favourites change at once on the TV, then on the server; a refusal puts them back.
  const [favorites, setFavorites] = useState<Record<number, boolean>>({});
  const channels = useMemo(
    () => (loaded.data ?? []).map((c) => (favorites[c.id] !== undefined && favorites[c.id] !== c.favorite ? { ...c, favorite: favorites[c.id]! } : c)),
    [loaded.data, favorites],
  );
  const toggleFavorite = (c: LiveChannel) => {
    const on = !c.favorite;
    setFavorites((f) => ({ ...f, [c.id]: on }));
    ui.api.setLiveFavorite(c.id, on).then(
      () => {
        ui.toast(on ? `${c.name} is a favourite` : `${c.name} isn't a favourite any more`);
        reload.current();
      },
      () => {
        setFavorites((f) => ({ ...f, [c.id]: !on }));
        ui.toast(on ? "Couldn't add it to your favourites" : "Couldn't take it out of your favourites", true);
      },
    );
  };

  const [view, setViewState] = useState<View>(() => (app.store.get(VIEW_KEY) === "channels" ? "channels" : "guide"));
  const setView = (v: View) => {
    app.store.set(VIEW_KEY, v);
    // A number typed before was for the view shown then.
    setJump(null);
    setViewState(v);
  };
  const [filter, setFilterState] = useState<ChannelFilter>(() => parseFilter(app.store.get(FILTER_KEY)));
  const setFilter = (f: ChannelFilter) => {
    app.store.set(FILTER_KEY, filterKey(f));
    entry.state.guideRow = undefined;
    entry.state.section = undefined;
    setFilterState(f);
  };
  const effective = effectiveFilter(filter, channels);
  const shown = useMemo(() => filterChannels(channels, effective), [channels, filterKey(effective)]);
  const groups = useMemo(() => groupsOf(channels), [channels]);
  const favoriteCount = channels.filter((c) => c.favorite).length;
  const recent = useMemo(() => (effective.kind === "all" ? recentChannels(channels, summary?.recent ?? [], RECENT_MAX) : []), [channels, summary?.recent, effective.kind]);
  const now = useNow();
  const [panel, setPanel] = useState<Panel>(null);
  const tabsRef = useRef<HTMLElement | null>(null);
  const chipsRef = useRef<HTMLElement | null>(null);

  const watch = (c: LiveChannel) => ui.nav.open({ name: "livePlayer", channelId: c.id, pressedAt: Date.now() });
  const details = (c: LiveChannel, program: LiveProgram | null) => setPanel({ kind: "details", channelId: c.id, program, opener: ui.focus.current?.getAttribute("data-fid") ?? null });
  const closePanel = () => {
    const opener = panel?.kind === "details" ? panel.opener : panel?.kind === "group" ? "chip/group" : null;
    setPanel(null);
    if (opener) {
      setTimeout(() => {
        const el = document.querySelector<HTMLElement>(`.live-page [data-fid="${opener}"]`);
        if (el) ui.focus.focus(el);
      }, 0);
    }
  };

  // A number typed: the guide or the cards go to that channel (in every channel when this filter hasn't it).
  const [jump, setJump] = useState<{ id: number; n: number } | null>(null);
  const [missing, setMissing] = useState<string | null>(null);
  const digits = useNumberEntry((d) => {
    const inShown = channelByNumber(shown, d);
    const any = inShown ?? channelByNumber(channels, d);
    if (!any) {
      setMissing(d);
      return;
    }
    if (!inShown) setFilter({ kind: "all" });
    setJump({ id: any.id, n: Date.now() });
  }, !panel);
  useEffect(() => {
    if (!missing) return;
    const t = setTimeout(() => setMissing(null), 2_000);
    return () => clearTimeout(t);
  }, [missing]);

  if (summary && !summary.allowed) {
    return (
      <div class="live-page">
        <ColumnMessage title="Live TV isn't open to this profile" body="An administrator can open it in Pley on the web: Settings → Live TV." />
      </div>
    );
  }
  if (!loaded.data) {
    if (loaded.error) {
      return (
        <div class="live-page">
          <ColumnMessage
            title={failedTitle(loaded.error, "Couldn't load Live TV")}
            body={problem(loaded.error)}
            action={{ label: "Retry", icon: "refresh-cw", onSelect: loaded.reload }}
          />
        </div>
      );
    }
    return <LiveSkeleton />;
  }
  if (!channels.length) {
    const admin = !!app.me.value?.is_admin;
    return (
      <div class="live-page">
        <ColumnMessage
          title={admin ? "Set up Live TV on the web" : "No channels yet"}
          body={
            admin
              ? "Add your IPTV provider's playlist or Xtream Codes account in Pley on the web: Settings → Live TV. Its channels and guide show here."
              : "Channels show here once an administrator adds an IPTV provider."
          }
        />
      </div>
    );
  }

  const group = effective.kind === "group" ? effective.group : null;
  const panelChannel = panel?.kind === "details" ? channels.filter((c) => c.id === panel.channelId)[0] ?? null : null;
  const focusTabs = () => {
    const el = tabsRef.current?.querySelector<HTMLElement>(`[data-fid="tab/${view}"]`);
    return !!el && ui.focus.focus(el);
  };
  return (
    <div class="live-page">
      <div class="library-header">
        <div class="t-page c-fg one-line" style="flex:1;min-width:0">
          Live TV
        </div>
        <G
          kind="row"
          class="library-tabs"
          elRef={(el) => (tabsRef.current = el)}
          onEnter={() => tabsRef.current?.querySelector<HTMLElement>(`[data-fid="tab/${view}"]`) ?? null}
        >
          <Tab fid="tab/guide" label="Guide" shown={view === "guide"} onSelect={() => setView("guide")} />
          <Tab fid="tab/channels" label="Channels" shown={view === "channels"} onSelect={() => setView("channels")} />
        </G>
      </div>
      <G kind="row" class="library-chips" elRef={(el) => (chipsRef.current = el)} onEnter={(dir) => (dir === "down" ? chipsRef.current?.querySelector<HTMLElement>("[data-f]") ?? null : null)}>
        <Chip fid="chip/all" label="All" selected={effective.kind === "all"} onSelect={() => setFilter({ kind: "all" })} />
        {favoriteCount > 0 && <Chip fid="chip/favorites" label="Favourites" icon="star" selected={effective.kind === "favorites"} onSelect={() => setFilter({ kind: "favorites" })} />}
        {groups.length > 1 && <Chip fid="chip/group" label={group ?? "Groups"} selected={!!group} maxWidth={220} onSelect={() => setPanel({ kind: "group" })} />}
        <div class="t-caption c-muted chips-count">{shown.length === 1 ? "1 channel" : `${shown.length} channels`}</div>
      </G>
      {!shown.length ? (
        <ColumnMessage
          top={146}
          title="No channels here"
          body={effective.kind === "favorites" ? "No favourites in this list." : "No channels in this group."}
          action={{ label: "Show every channel", icon: "list", onSelect: () => setFilter({ kind: "all" }) }}
        />
      ) : view === "guide" ? (
        <LiveGuide
          key={filterKey(effective)}
          entry={entry}
          channels={shown}
          now={now}
          jump={jump}
          hours24={app.platform.clock24()}
          onWatch={watch}
          onDetails={details}
          focusAbove={focusTabs}
          version={app.liveChanges.value}
        />
      ) : (
        <ChannelCards key={filterKey(effective)} entry={entry} channels={shown} recent={recent} title={group ?? (effective.kind === "favorites" ? "Favourites" : "All channels")} now={now} jump={jump} onWatch={watch} onDetails={details} />
      )}
      <NumberBox digits={digits} missing={missing} />
      {panel?.kind === "group" && (
        <SidePanel onClose={closePanel} label="Groups">
          <PanelTitle>Groups</PanelTitle>
          <PanelRow label="All channels" sub={`${channels.length} channels`} checked={!group} initial={!group} onSelect={() => (setFilter({ kind: "all" }), closePanel())} />
          {groups.map((g) => (
            <PanelRow
              key={g.name}
              label={g.name}
              sub={g.count === 1 ? "1 channel" : `${g.count} channels`}
              checked={g.name === group}
              initial={g.name === group}
              onSelect={() => {
                setFilter({ kind: "group", group: g.name });
                closePanel();
              }}
            />
          ))}
        </SidePanel>
      )}
      {panel?.kind === "details" && panelChannel && (
        <ProgramPanel
          channel={panelChannel}
          program={panel.program}
          now={now}
          hours24={app.platform.clock24()}
          onClose={closePanel}
          onWatch={() => {
            setPanel(null);
            watch(panelChannel);
          }}
          onFavorite={() => toggleFavorite(panelChannel)}
        />
      )}
    </div>
  );
}

/** Live TV while its channels come (after 300 ms): grey blocks where the header and the guide go. */
function LiveSkeleton() {
  const ui = useUi();
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => {
      setSlow(true);
      if (!ui.focus.current || !ui.focus.current.isConnected) ui.toRail();
    }, 300);
    return () => clearTimeout(t);
  }, []);
  if (!slow) return <div class="live-page" />;
  const block = (x: number, y: number, w: number, h: number) => <div class="skeleton" style={`position:absolute;left:${x}px;top:${y}px;width:${w}px;height:${h}px`} />;
  return (
    <div class="live-page" role="status" aria-label="Loading Live TV">
      {block(96, 32, 160, 30)}
      {block(96, 80, 72, 40)}
      {block(178, 80, 120, 40)}
      {[0, 1, 2, 3, 4, 5].map((i) => (
        <div key={i}>
          {block(96, 172 + i * 58, 184, 52)}
          {block(288, 172 + i * 58, 240, 52)}
          {block(532, 172 + i * 58, 380, 52)}
        </div>
      ))}
    </div>
  );
}

// ---- a programme's details

/** A programme (or a channel without one) in a side panel: what the guide says of it, Watch, and the channel's favourite. */
function ProgramPanel(p: {
  channel: LiveChannel;
  program: LiveProgram | null;
  now: number;
  hours24: boolean;
  onClose: () => void;
  onWatch: () => void;
  onFavorite: () => void;
}) {
  const c = p.channel;
  const prog = p.program;
  const on = prog ? airing(prog, p.now) : true;
  const ended = !!prog && prog.stop <= p.now;
  const episode = prog ? episodeText(prog) : null;
  const sub = prog ? [episode, prog.subtitle].filter(Boolean).join(" · ") : "";
  return (
    <SidePanel onClose={p.onClose} label={prog ? prog.title : c.name} wide>
      <div class="live-panel-channel">
        <ChannelLogo channel={c} width={72} height={40} />
        <div class="t-label c-muted one-line" style="margin-left:12px;min-width:0">
          {c.number ? `${c.number}  ${c.name}` : c.name}
        </div>
      </div>
      {prog ? (
        <div class="live-panel-text">
          <div class="t-title c-fg lines-2">{prog.title}</div>
          {sub && <div class="t-label c-muted one-line" style="margin-top:4px">{sub}</div>}
          <div class="live-panel-facts">
            <span class="t-caption c-muted nowrap">{`${formatGuideDay(prog.start, p.now)} · ${formatSpan(prog, p.hours24)}`}</span>
            <ProgramMarks program={prog} />
            {prog.rating && <PageBadge text={prog.rating} />}
          </div>
          {on && (
            <div class="live-panel-progress">
              <ProgressLine fraction={programProgress(prog, p.now)} />
              <span class="t-caption c-muted nowrap" style="margin-left:10px">
                {timeLeft(prog, p.now)}
              </span>
            </div>
          )}
          <div class={`t-body ${prog.description ? "c-fg" : "c-subtle"} lines-6`} style="margin-top:10px">
            {prog.description || "The guide says nothing more of it."}
          </div>
          {prog.categories.length > 0 && <div class="t-caption c-subtle one-line" style="margin-top:8px">{prog.categories.join(" · ")}</div>}
          {ended && <div class="t-caption c-subtle" style="margin-top:8px">It has ended.</div>}
        </div>
      ) : (
        <div class="live-panel-text">
          <div class="t-title c-fg">{c.name}</div>
          <div class="t-body c-subtle" style="margin-top:6px">
            {c.has_guide ? "The guide says nothing of this time." : "The guide has nothing for this channel."}
          </div>
        </div>
      )}
      <PanelRow icon="play" label={on ? "Watch now" : "Watch channel"} initial onSelect={p.onWatch} />
      <PanelRow icon={c.favorite ? "star" : "star-outline"} label={c.favorite ? "Remove from favourites" : "Add to favourites"} onSelect={p.onFavorite} />
    </SidePanel>
  );
}

// ---- the channels as cards

/** A card's height, the row's gap under it (SectionPage's 28), and the header over a titled row. */
const CARD_H = 92;
const SECTION_HEADER = 24 + 14;
/** Where the focused row's cards sit: under the chips, as the library's grid. */
const CARDS_TOP = 172;
/** Nothing shows above a titled row's header, nor 12 above an untitled row's cards (the row before's end 28 above). */
const CARDS_CLIP = 124;
const CARDS_CLIP_UNTITLED = CARDS_TOP - 12;
/** Two cards a row, 400 wide, 16 apart (96 to 912). */
const CARD_W = 400;
const CARD_GAP = 16;

/**
 * The channels as cards, two a row (the web's ChannelGrid on a TV): the
 * logo, the channel, what's on with how far it is, and what's next. The
 * ones watched last come first, in a row that slides.
 */
function ChannelCards(p: {
  entry: Entry;
  channels: LiveChannel[];
  recent: LiveChannel[];
  title: string;
  now: number;
  jump: { id: number; n: number } | null;
  onWatch: (c: LiveChannel) => void;
  onDetails: (c: LiveChannel, program: LiveProgram | null) => void;
}) {
  const ui = useUi();
  const entry = p.entry;
  const [recentAt, setRecentAt] = useState(0);
  const [focusedKey, setFocusedKey] = useState<string | undefined>(entry.state.section as string | undefined);
  /** The section focus is in: a row without a header is cut closer above. */
  const [at, setAt] = useState<string | undefined>(focusedKey);
  const hours24 = useApp().platform.clock24();
  const rows: LiveChannel[][] = [];
  for (let i = 0; i < p.channels.length; i += 2) rows.push(p.channels.slice(i, i + 2));

  // A number typed: its card's row comes into view, and its card takes focus.
  useEffect(() => {
    if (!p.jump) return;
    const at = p.channels.findIndex((c) => c.id === p.jump!.id);
    if (at < 0) return;
    const key = `row/${Math.floor(at / 2)}`;
    entry.state.section = key;
    setFocusedKey(key);
    setAt(key);
    setTimeout(() => {
      const el = document.querySelector<HTMLElement>(`.live-page [data-fid="card/${p.jump!.id}"]`);
      if (el) ui.focus.focus(el);
    }, 0);
  }, [p.jump?.n]);

  const card = (c: LiveChannel, fid: string, initial: boolean, onFocus: () => void) => (
    <ChannelCard key={c.id} channel={c} fid={fid} initial={initial} now={p.now} hours24={hours24} onFocus={onFocus} onWatch={() => p.onWatch(c)} onDetails={(prog) => p.onDetails(c, prog)} />
  );
  const sections: Section[] = [];
  if (p.recent.length) {
    const shift = Math.max(0, Math.min(recentAt - 1, p.recent.length - 2)) * (CARD_W + CARD_GAP);
    sections.push({
      key: "recent",
      height: SECTION_HEADER + CARD_H,
      anchor: CARDS_TOP - SECTION_HEADER,
      render: (onFocus) => (
        <>
          <div class="row-header t-section one-line">Recently watched</div>
          <div class="live-card-viewport">
            <G kind="row" restore class="live-card-track" style={`transform:translateX(${-shift}px)`}>
              {p.recent.map((c, i) =>
                card(c, `recent/${c.id}`, i === 0, () => {
                  setRecentAt(i);
                  onFocus();
                }),
              )}
            </G>
          </div>
        </>
      ),
    });
  }
  rows.forEach((row, r) => {
    const titled = r === 0;
    sections.push({
      key: `row/${r}`,
      height: (titled ? SECTION_HEADER : 0) + CARD_H,
      anchor: titled ? CARDS_TOP - SECTION_HEADER : CARDS_TOP,
      render: (onFocus) => (
        <>
          {titled && <div class="row-header t-section one-line">{p.title}</div>}
          <G kind="row" class="live-card-row">
            {row.map((c, i) => card(c, `card/${c.id}`, r === 0 && i === 0 && !p.recent.length, onFocus))}
          </G>
        </>
      ),
    });
  });
  return (
    <SectionPage
      sections={sections}
      top={CARDS_TOP - SECTION_HEADER}
      clipAbove={at && at !== "recent" && at !== "row/0" ? CARDS_CLIP_UNTITLED : CARDS_CLIP}
      focusedKey={focusedKey}
      onFocusSection={(key) => {
        entry.state.section = key;
        setAt(key);
      }}
    />
  );
}

/** One channel as a card: the logo, its number and name, what's on with its progress, and what's next. */
function ChannelCard(p: {
  channel: LiveChannel;
  fid: string;
  initial: boolean;
  now: number;
  hours24: boolean;
  onFocus: () => void;
  onWatch: () => void;
  onDetails: (program: LiveProgram | null) => void;
}) {
  const c = p.channel;
  const { on, next } = nowAndNext(c, p.now);
  const label = `${c.number ? `${c.number} ` : ""}${c.name}${on ? `, now: ${on.title}` : ""}`;
  return (
    <F class="live-card" fid={p.fid} initial={p.initial} label={label} onFocus={p.onFocus} onSelect={p.onWatch} onLongSelect={() => p.onDetails(on)} onMenu={() => p.onDetails(on)}>
      <ChannelLogo channel={c} width={120} height={68} class="live-card-logo" showName />
      <div class="live-card-text">
        <div class="live-card-name">
          {c.number && <span class="t-label live-number">{c.number}</span>}
          <span class="t-label one-line live-card-title">{c.name}</span>
          {c.favorite && <Icon name="star" size={14} class="live-star" />}
        </div>
        {on ? (
          <>
            <div class="t-body one-line live-card-now">{on.title}</div>
            <div class="live-card-progress">
              <ProgressLine fraction={programProgress(on, p.now)} />
              <span class="t-caption live-card-sub nowrap" style="margin-left:8px">
                {formatTime(on.stop, p.hours24)}
              </span>
            </div>
          </>
        ) : (
          <div class="t-body one-line live-card-sub">{c.has_guide ? "Nothing in the guide now" : "No guide for this channel"}</div>
        )}
        {next && <div class="t-caption one-line live-card-sub">{`${formatTime(next.start, p.hours24)} · ${next.title}`}</div>}
      </div>
    </F>
  );
}
