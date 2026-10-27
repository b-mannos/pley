import { useEffect, useMemo, useState } from "preact/hooks";
import { errorMessage, isApiError } from "../../api/client";
import type { ItemSummary, Playlist, PlaylistEntry } from "../../api/types";
import { cardSubtitle, episodeLabel, formatDuration, pluralize, seasonTitle } from "../../format/format";
import { useUi } from "../context";
import { Art, artCandidates } from "../kit/art";
import { Backdrop } from "../kit/backdrop";
import { Pill, PanelRow, Round } from "../kit/buttons";
import { TextField } from "../kit/field";
import { F, G, railOnLeft } from "../kit/focusable";
import { PosterGrid } from "../kit/grid";
import { Icon } from "../kit/icons";
import { PanelHeading, PanelTitle, SidePanel } from "../kit/panel";
import { describeSmartFilter } from "./library-data";
import { ArtTile } from "../kit/tile";
import type { Entry } from "../shell/navigator";
import { ColumnMessage, Dialog, failedTitle, PageBadge, problem, sentence, useApp, useLoad } from "./common";
import { openPlayer, pageOf } from "./item-actions";
import { musicPlayer } from "../../player/music";

/** "12 items · 3h 10m". */
export function playlistMeta(p: Playlist): string {
  return [pluralize(p.item_count, p.kind === "audio" ? "track" : "item"), p.duration_ms ? formatDuration(p.duration_ms) : ""].filter(Boolean).join(" · ");
}

/**
 * Playlists (the rail's page): New playlist at the title's right (with
 * none yet, it takes focus), then every playlist as a tile.
 */
export function PlaylistsScreen(p: { entry: Entry }) {
  const ui = useUi();
  const loaded = useLoad((c) => ui.api.playlists(c), []);
  const list = loaded.data?.playlists ?? null;
  if (!list && loaded.error) {
    return <ColumnMessage top={80} title={failedTitle(loaded.error, "Couldn't load your playlists")} body={problem(loaded.error)} action={{ label: "Retry", icon: "refresh-cw", onSelect: loaded.reload }} />;
  }
  if (!list) return null;
  return (
    <div class="playlists-page">
      <div class="t-page c-fg" style="position:absolute;left:96px;top:27px">
        Playlists
      </div>
      <G kind="row" class="playlists-new" onEdge={railOnLeft(ui.toRail)}>
        <Pill label="New playlist" icon="plus" fid="new" initial={!list.length} onSelect={() => ui.nav.open({ name: "newPlaylist" })} />
      </G>
      {!list.length ? (
        <ColumnMessage top={80} title="No playlists yet" body="Make one here, or use “Add to playlist” on a movie, show, album or episode (also when holding OK on a poster)." />
      ) : (
        <PosterGrid
          kind="square"
          count={list.length}
          top={96}
          clipTop={82}
          startIndex={(p.entry.state.index as number | undefined) ?? 0}
          onFocusIndex={(i) => (p.entry.state.index = i)}
          cell={(i, place) => {
            const pl = list[i]!;
            return (
              <ArtTile
                kind="square"
                title={pl.title}
                subtitle={playlistMeta(pl)}
                candidates={pl.artwork}
                fid={`playlist/${pl.id}`}
                initial={i === 0}
                priority={place.priority}
                onFocus={place.onFocus}
                onSelect={() => ui.nav.open({ name: "playlist", id: pl.id })}
              />
            );
          }}
        />
      )}
      <div class="rail-fade" />
    </div>
  );
}

/** The page's top, down to just above the first entry (design spec §4.3: what is below starts at 452). */
const HERO_H = 436;
const LIST_TOP = HERO_H + 16;
/** An entry's row: its 90 px still, 8 above and below. */
const ROW_H = 106;
/** Where a focused entry's top settles once the page has scrolled. */
const ROW_ANCHOR = 140;

/** An entry's two lines (the Android app's Playlists.rowTitles): an episode's own title over its show and number. */
export function entryTitles(item: ItemSummary): [string, string] {
  if (item.kind === "episode") return [item.title, [item.grandparent_title ?? "", episodeLabel(item.parent_index, item.index)].filter(Boolean).join(" · ")];
  if (item.kind === "season") return [seasonTitle(item.title, item.index), cardSubtitle(item)];
  return [item.title, cardSubtitle(item)];
}

/** What can be open over the page: an entry's actions, Rename, or Delete's question. */
type Overlay = null | { actions: PlaylistEntry } | "rename" | "delete";

/**
 * A playlist's page (TvPlaylistScreen): the item page's top (the first
 * entry's backdrop; what it plays, the title, "12 items · 3h 12m" and a
 * smart playlist's filter in words; Play, Shuffle, Rename, Delete), then
 * its entries: number, still, title and length, and More. OK on an entry
 * plays from it; holding OK, Menu or More opens its actions: Play from
 * here, Go to it, Move, Remove. Move lifts the entry: Up and Down move
 * it, OK drops it there (and sends the order), Back puts it back. A smart
 * playlist fills itself: nothing in it is edited here.
 */
export function PlaylistScreen(p: { entry: Entry; id: number }) {
  const ui = useUi();
  const loaded = useLoad((c) => ui.api.playlist(p.id, c), [p.id]);
  const pl = loaded.data;
  if (!pl) {
    if (loaded.error) {
      if (isApiError(loaded.error, 404)) {
        return <ColumnMessage top={58} title="Playlist not found" body="It was deleted, or it belongs to another profile." action={{ label: "Back", onSelect: () => ui.nav.pop() }} />;
      }
      return <ColumnMessage top={58} title={failedTitle(loaded.error, "Couldn't load this playlist")} body={problem(loaded.error)} action={{ label: "Retry", icon: "refresh-cw", onSelect: loaded.reload }} />;
    }
    return <PlaylistSkeleton />;
  }
  return <PlaylistPage entry={p.entry} pl={pl} reload={loaded.reload} />;
}

function PlaylistPage(p: { entry: Entry; pl: Playlist & { entries: PlaylistEntry[] }; reload: () => void }) {
  const ui = useUi();
  const pl = p.pl;
  const video = pl.kind !== "audio";
  const smart = !!pl.smart_filter;
  const editable = !smart;
  const [title, setTitle] = useState(pl.title);
  useEffect(() => setTitle(pl.title), [pl.title]);
  // Entries taken out here and on their way to the server; the order moved to and not yet sent.
  const [gone, setGone] = useState<number[]>([]);
  const [order, setOrder] = useState<number[] | null>(null);
  const [moving, setMoving] = useState<number | null>(null);
  const [overlay, setOverlay] = useState<Overlay>(null);
  /** The focused entry's index; -1 in the top. */
  const [at, setAt] = useState<number>((p.entry.state.index as number | undefined) ?? -1);
  // The server's answer holds the order sent: it shows as it is.
  useEffect(() => {
    if (moving === null) setOrder(null);
  }, [pl.entries]);
  const entries = useMemo(() => {
    let list = pl.entries.filter((e) => gone.indexOf(e.entry_id) < 0);
    if (order) {
      const pos = new Map(order.map((id, i) => [id, i] as [number, number]));
      list = list.slice().sort((a, b) => (pos.get(a.entry_id) ?? 1e9) - (pos.get(b.entry_id) ?? 1e9));
    }
    return list;
  }, [pl.entries, gone, order]);
  const left = pl.entries.filter((e) => gone.indexOf(e.entry_id) >= 0);
  const meta = playlistMeta({
    ...pl,
    item_count: Math.max(0, pl.item_count - left.length),
    duration_ms: Math.max(0, pl.duration_ms - left.reduce((n, e) => n + (e.item.duration_ms ?? 0), 0)),
  });
  const facts = [meta].concat(smart ? describeSmartFilter(pl.smart_filter ?? "") : []).filter(Boolean).join("  ·  ");
  const canPlay = entries.length > 0;
  const lead = entries[0]?.item ?? pl.entries[0]?.item ?? null;

  const play = (from: number, shuffle = false) => {
    if (!entries.length) return;
    if (!video) {
      musicPlayer().play(entries.map((e) => e.item), from, shuffle);
      ui.nav.open({ name: "nowPlaying" });
      return;
    }
    const start = shuffle ? Math.floor(Math.random() * entries.length) : from;
    openPlayer(ui, { itemId: entries[start]!.item.id, queue: { kind: "playlist", id: pl.id } });
  };
  const focusFid = (fid: string) =>
    setTimeout(() => {
      const el = document.querySelector<HTMLElement>(`.playlist-page [data-fid="${fid}"]`);
      if (el) ui.focus.focus(el);
    }, 0);
  const moveBy = (id: number, by: number) => {
    const ids = entries.map((e) => e.entry_id);
    const from = ids.indexOf(id);
    const to = from + by;
    if (from < 0 || to < 0 || to >= ids.length) return;
    ids.splice(to, 0, ids.splice(from, 1)[0]!);
    setOrder(ids);
    setAt(to);
    p.entry.state.index = to;
  };
  const drop = () => {
    setMoving(null);
    const ids = order;
    if (!ids) return;
    ui.api.reorderPlaylist(pl.id, ids).then(p.reload, (e) => {
      setOrder(null);
      ui.toast(sentence(errorMessage(e)), true);
    });
  };
  const putBack = () => {
    setMoving(null);
    setOrder(null);
  };
  const remove = (e: PlaylistEntry) => {
    const i = entries.findIndex((x) => x.entry_id === e.entry_id);
    const next = entries[i + 1] ?? entries[i - 1] ?? null;
    setGone((g) => g.concat([e.entry_id]));
    ui.toast(`Removed “${entryTitles(e.item)[0]}”`);
    if (next) focusFid(`entry/${next.entry_id}`);
    else focusFid(editable ? "rename" : "delete");
    ui.api.removeFromPlaylist(pl.id, e.entry_id).then(p.reload, (err) => {
      setGone((g) => g.filter((x) => x !== e.entry_id));
      ui.toast(sentence(errorMessage(err)), true);
    });
  };
  const rename = (typed: string) => {
    const name = typed.trim();
    if (!name || name.length > TITLE_MAX) return;
    setOverlay(null);
    if (name === title) return;
    const before = title;
    setTitle(name);
    ui.api.renamePlaylist(pl.id, name).then(p.reload, (e) => {
      setTitle(before);
      ui.toast(sentence(errorMessage(e)), true);
    });
  };
  const del = () => {
    setOverlay(null);
    ui.api.deletePlaylist(pl.id).then(
      () => ui.nav.pop(),
      (e) => ui.toast(sentence(errorMessage(e)), true),
    );
  };

  // The column scrolls with focus: the top at rest in it, a focused entry's top at its line.
  const shift = at >= 0 ? Math.max(0, LIST_TOP + at * ROW_H - ROW_ANCHOR) : 0;
  const first = Math.max(0, Math.floor((shift - LIST_TOP) / ROW_H) - 2);
  const last = Math.min(entries.length, first + Math.ceil(540 / ROW_H) + 4);
  const rows = [];
  for (let i = first; i < last; i++) {
    const e = entries[i]!;
    rows.push(
      <EntryRow
        key={e.entry_id}
        e={e}
        index={i}
        video={video}
        moving={moving === e.entry_id}
        toRail={ui.toRail}
        onFocus={() => {
          setAt(i);
          p.entry.state.index = i;
        }}
        onPlay={() => play(i)}
        onMore={() => setOverlay({ actions: e })}
        onMove={(by) => moveBy(e.entry_id, by)}
        onDrop={drop}
        onPutBack={putBack}
      />,
    );
  }
  const topFocus = () => {
    setAt(-1);
    p.entry.state.index = -1;
  };
  const acting = overlay && typeof overlay === "object" ? overlay.actions : null;
  const actingTitles = acting ? entryTitles(acting.item) : null;
  return (
    <div class="playlist-page">
      <Backdrop candidates={lead ? artCandidates(lead, "backdrop") : []} title={title} scrim="item" />
      {/* The list passes over the backdrop: it darkens once the top scrolls away. */}
      <div class={`playlist-dim${shift > 0 ? " in" : ""}`} />
      <div class="playlist-content" style={`transform:translateY(${-shift}px)`}>
        <div class="playlist-hero">
          <div class="playlist-kind t-caption">
            <Icon name={video ? "list-video" : "list-music"} size={18} />
            <span>{smart ? "Smart playlist" : video ? "Video playlist" : "Music playlist"}</span>
          </div>
          <div class="t-hero c-fg lines-2" style="margin-top:10px">
            {title}
          </div>
          <div class="playlist-facts" style="margin-top:12px">
            {smart && <PageBadge text="Smart" />}
            <span class="t-label c-muted lines-2">{facts}</span>
          </div>
        </div>
        <G kind="row" restore class="buttons playlist-buttons" style={`top:${canPlay ? 361 : 341}px`} onEdge={railOnLeft(ui.toRail)}>
          <Pill label="Play" icon="play" fid="play" initial={canPlay} disabled={!canPlay} onFocus={topFocus} onSelect={() => play(0)} />
          <Round label="Shuffle" icon="shuffle" fid="shuffle" disabled={entries.length < 2} onFocus={topFocus} onSelect={() => play(0, true)} />
          {editable && <Round label="Rename" icon="pencil" fid="rename" initial={!canPlay} onFocus={topFocus} onSelect={() => setOverlay("rename")} />}
          <Round label="Delete" icon="trash-2" fid="delete" initial={!canPlay && !editable} onFocus={topFocus} onSelect={() => setOverlay("delete")} />
        </G>
        {!canPlay && (
          <div class="t-body c-muted playlist-empty">
            {smart
              ? "Nothing matches this playlist's filter yet."
              : video
                ? "This playlist is empty. Use “Add to playlist” on a movie, show or episode, or hold OK on a poster."
                : "This playlist is empty. Use “Add to playlist” on an artist or album, or hold OK on a cover."}
          </div>
        )}
        <G kind="col" restore class="playlist-entries" style={`top:${LIST_TOP}px;height:${entries.length * ROW_H}px`}>
          {rows}
        </G>
      </div>
      <div class="rail-fade" />
      {moving !== null && <div class="moving-hint t-label">Up and down to move it, OK to drop it here, Back to put it back</div>}
      {acting && actingTitles && (
        <SidePanel onClose={() => setOverlay(null)} label={actingTitles[0]}>
          <div class="panel-title">
            <div class="t-title c-fg lines-2">{actingTitles[0]}</div>
            {actingTitles[1] && <div class="t-caption c-muted one-line">{actingTitles[1]}</div>}
          </div>
          <PanelRow
            label="Play from here"
            icon="play"
            initial
            onSelect={() => {
              setOverlay(null);
              play(Math.max(0, entries.findIndex((x) => x.entry_id === acting.entry_id)));
            }}
          />
          <PanelRow
            label={`Go to ${actingTitles[0]}`}
            icon="arrow-right"
            onSelect={() => {
              setOverlay(null);
              ui.nav.open(pageOf(acting.item));
            }}
          />
          {editable && (
            <PanelRow
              label="Move"
              icon="arrow-up-down"
              disabled={entries.length < 2}
              onSelect={() => {
                setOverlay(null);
                setMoving(acting.entry_id);
                focusFid(`entry/${acting.entry_id}`);
              }}
            />
          )}
          {editable && (
            <PanelRow
              label="Remove from playlist"
              icon="x"
              danger
              onSelect={() => {
                setOverlay(null);
                remove(acting);
              }}
            />
          )}
        </SidePanel>
      )}
      {overlay === "rename" && (
        <SidePanel onClose={() => setOverlay(null)} label="Rename playlist">
          <PanelTitle>Rename playlist</PanelTitle>
          <RenameForm current={title} onSave={rename} />
        </SidePanel>
      )}
      {overlay === "delete" && (
        <Dialog
          title="Delete playlist?"
          body={`“${title}” will be deleted. What's in it stays in your libraries.`}
          onDismiss={() => setOverlay(null)}
          buttons={[
            { label: "Cancel", initial: true, onSelect: () => setOverlay(null) },
            { label: "Delete", color: "var(--danger-text)", onSelect: del },
          ]}
        />
      )}
    </div>
  );
}

/**
 * One entry (design spec §2.7: rows fill without scaling): its number,
 * 160 × 90 still (90 × 90 cover for music), its two lines and length;
 * focused, an accent fill. More sits at its end, a Right away. While
 * `moving` it is lifted (a ring, the arrows): Up and Down move it, OK
 * drops it, Back puts it back.
 */
function EntryRow(p: {
  e: PlaylistEntry;
  index: number;
  video: boolean;
  moving: boolean;
  toRail: () => boolean;
  onFocus: () => void;
  onPlay: () => void;
  onMore: () => void;
  onMove: (by: -1 | 1) => void;
  onDrop: () => void;
  onPutBack: () => void;
}) {
  const item = p.e.item;
  const [title, subtitle] = entryTitles(item);
  const duration = formatDuration(item.duration_ms);
  return (
    <G kind="row" class="entry-row" style={`top:${p.index * ROW_H}px`} onEdge={railOnLeft(p.toRail)}>
      <F
        class={`entry${p.moving ? " moving" : ""}`}
        fid={`entry/${p.e.entry_id}`}
        label={`${p.index + 1}. ${[title, subtitle, duration].filter(Boolean).join(", ")}`}
        onFocus={p.onFocus}
        onSelect={p.onPlay}
        onLongSelect={p.onMore}
        onMenu={p.onMore}
        onKey={(key) => {
          if (!p.moving) return false;
          if (key === "up" || key === "down") {
            p.onMove(key === "up" ? -1 : 1);
            return true;
          }
          if (key === "ok:up") {
            p.onDrop();
            return true;
          }
          if (key === "back") {
            p.onPutBack();
            return true;
          }
          return key === "ok" || key === "left" || key === "right" || key.indexOf(":up") > 0;
        }}
      >
        <div class="entry-number t-label">{p.index + 1}</div>
        <div class={`entry-still${p.video ? "" : " square"}`}>
          <Art candidates={artCandidates(item, p.video ? "thumb" : "poster")} width={p.video ? 160 : 90} title={item.title} class="tile-clip" />
        </div>
        <div class="entry-text">
          <div class="t-title one-line">{title}</div>
          {subtitle && <div class="t-caption one-line entry-sub">{subtitle}</div>}
        </div>
        {p.moving ? (
          <div class="entry-arrows" aria-hidden="true">
            <Icon name="chevron-up" size={22} />
            <Icon name="chevron-down" size={22} />
          </div>
        ) : duration ? (
          <div class="t-caption entry-sub entry-duration">{duration}</div>
        ) : null}
      </F>
      <div class="entry-more">
        <Round label="More" icon="ellipsis" fid={`more/${p.e.entry_id}`} offArt disabled={p.moving} onSelect={p.onMore} />
      </div>
    </G>
  );
}

/** Rename: the field with the title as it is (OK types), and Save once it will do. */
function RenameForm(p: { current: string; onSave: (title: string) => void }) {
  const [name, setName] = useState(p.current);
  const ok = name.trim().length > 0 && name.trim().length <= TITLE_MAX;
  return (
    <>
      <div style="padding:6px 4px">
        <TextField value={name} placeholder="Title" initial onInput={setName} onSubmit={(v) => p.onSave(v)} />
      </div>
      <div class="t-caption c-muted" style="padding:4px 0 0 8px">
        Press OK on the title to type it.
      </div>
      <G kind="row" style="padding:16px 4px 0">
        <Pill label="Save" fid="save" disabled={!ok} onSelect={() => p.onSave(name)} />
      </G>
    </>
  );
}

/** Still blocks in the top's shapes while the playlist loads. */
function PlaylistSkeleton() {
  const b = (top: number, w: number, h: number, r = 2) => <div class="skeleton" style={`position:absolute;left:96px;top:${top}px;width:${w}px;height:${h}px;border-radius:${r}px`} />;
  return (
    <div class="playlist-page" role="status" aria-label="Loading">
      {b(58, 120, 18)}
      {b(90, 360, 44)}
      {b(148, 200, 18)}
      {b(361, 128, 40, 6)}
    </div>
  );
}

/** A line of a track list (TvTrackLine): number, title and subtitle, duration; the playing one marked. */
export function TrackLine(p: {
  fid?: string;
  number: string;
  title: string;
  subtitle?: string;
  duration: string;
  current?: boolean;
  playing?: boolean;
  highlighted?: boolean;
  initial?: boolean;
  onSelect: () => void;
  onActions?: () => void;
}) {
  return (
    <F
      class={`track-line${p.current ? " current" : ""}${p.highlighted ? " highlighted" : ""}`}
      fid={p.fid}
      initial={p.initial}
      label={[p.title, p.subtitle].filter(Boolean).join(", ")}
      onSelect={p.onSelect}
      onLongSelect={p.onActions}
      onMenu={p.onActions}
    >
      <div class="track-number t-caption">{p.current ? <Icon name={p.playing ? "audio-lines" : "play"} size={18} /> : p.number}</div>
      <div class="track-text">
        <div class="t-body one-line track-title">{p.title}</div>
        {p.subtitle && <div class="t-caption one-line track-sub">{p.subtitle}</div>}
      </div>
      <div class="t-caption track-sub">{p.duration}</div>
    </F>
  );
}

/** A playlist's title is 1 to 200 characters (the Android app's Playlists.check). */
const TITLE_MAX = 200;

/**
 * New playlist and Save these filters (TvNewPlaylistScreen): what it is on
 * the left; a panel with the title (OK types it) and, for an empty
 * playlist, what it plays. Create, or Save, makes it and opens it in this
 * page's place.
 */
export function NewPlaylistScreen(p: { smart?: { libraryId: number; filter: string } }) {
  const ui = useUi();
  const smart = p.smart ?? null;
  const [name, setName] = useState("");
  const [kind, setKind] = useState<"video" | "audio">("video");
  const [busy, setBusy] = useState(false);
  const heading = smart ? "Save these filters" : "New playlist";
  const filters = smart ? describeSmartFilter(smart.filter) : [];
  const body = !smart
    ? "Name it, then add to it with “Add to playlist” on any title."
    : filters.length
      ? `${filters.join(" · ")}. It keeps up with the library by itself.`
      : "It keeps up with the library by itself.";
  const fits = (t: string) => t.length > 0 && t.length <= TITLE_MAX;
  const create = (typed?: string) => {
    const title = (typed ?? name).trim();
    if (!fits(title) || busy) return;
    setBusy(true);
    const made = smart ? ui.api.createSmartPlaylist(title, smart.libraryId, smart.filter) : ui.api.createPlaylist(title, kind, []);
    made.then(
      (pl) => ui.nav.replace({ name: "playlist", id: pl.id }),
      (e) => {
        setBusy(false);
        ui.toast(sentence(errorMessage(e)), true);
      },
    );
  };
  return (
    <div class="sheet-page">
      <div class="sheet-left">
        <Icon name="list-plus" size={32} color="var(--muted)" />
        <div class="t-page c-fg" style="margin-top:16px">
          {heading}
        </div>
        <div class="t-body-large c-muted" style="margin-top:12px">
          {body}
        </div>
      </div>
      <SidePanel onClose={() => ui.nav.pop()} label={heading}>
        <PanelTitle>{heading}</PanelTitle>
        <div style="padding:6px 4px">
          <TextField value={name} placeholder="Title" initial onInput={setName} onSubmit={(v) => create(v)} />
        </div>
        <div class="t-caption c-muted" style="padding:4px 0 0 8px">
          Press OK on the title to type it.
        </div>
        {!smart && (
          <>
            <PanelHeading>For</PanelHeading>
            <PanelRow label="Movies & TV" icon="list-video" checked={kind === "video"} onSelect={() => setKind("video")} />
            <PanelRow label="Music" icon="list-music" checked={kind === "audio"} onSelect={() => setKind("audio")} />
          </>
        )}
        <div style="height:16px" />
        <PanelRow label={smart ? "Save" : "Create"} icon={smart ? "save" : "plus"} disabled={!fits(name.trim()) || busy} onSelect={() => create()} />
      </SidePanel>
    </div>
  );
}

/**
 * Add to playlist (the sheet): a name for a new playlist, then the
 * playlists of the same kind. Closes itself 1.4 s after adding.
 */
export function AddToPlaylistScreen(p: { itemIds: number[]; title: string }) {
  const app = useApp();
  const ui = useUi();
  const [kind, setKind] = useState<"video" | "audio" | null>(null);
  useEffect(() => {
    const first = p.itemIds[0];
    if (first === undefined) return;
    const cached = app.cachedItem(first);
    const decide = (k: string) => setKind(k === "track" || k === "album" || k === "artist" ? "audio" : "video");
    if (cached) decide(cached.kind);
    else ui.api.item(first).then((d) => decide(d.kind), () => setKind("video"));
  }, []);
  const loaded = useLoad((c) => ui.api.playlists(c), []);
  const [name, setName] = useState("");
  const [done, setDone] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const finish = (message: string) => {
    setDone(message);
    setTimeout(() => ui.nav.pop(), 1400);
  };
  const add = (pl: Playlist) => {
    ui.api.addToPlaylist(pl.id, p.itemIds).then(
      (r) => finish(r.added === 0 ? "Nothing to add" : r.added === 1 ? `Added to ${pl.title}` : `Added ${r.added} items to ${pl.title}`),
      (e) => setError(sentence(errorMessage(e))),
    );
  };
  const create = () => {
    const title = name.trim();
    if (!title || title.length > 200) return;
    ui.api.createPlaylist(title, kind, p.itemIds).then(
      (pl) => finish(`Added to ${pl.title}`),
      (e) => setError(sentence(errorMessage(e))),
    );
  };
  const lists = (loaded.data?.playlists ?? []).filter((pl) => !pl.smart_filter && (!kind || pl.kind === kind));
  return (
    <div class="sheet-page">
      <div class="sheet-left">
        <Icon name="list-plus" size={32} color="var(--muted)" />
        <div class="t-label c-muted" style="margin-top:16px">
          Add to playlist
        </div>
        <div class="t-page c-fg lines-3" style="margin-top:6px">
          {p.title}
        </div>
      </div>
      <SidePanel onClose={() => ui.nav.pop()} label="Add to playlist">
        {done ? (
          <div class="sheet-done">
            <Icon name="circle-check" size={48} color="var(--accent)" />
            <div class="t-title c-fg" style="margin-top:16px">
              {done}
            </div>
          </div>
        ) : (
          <>
            <div class="panel-title">
              <div class="t-title c-fg">Add to playlist</div>
              <div class="t-caption c-muted one-line">{p.title}</div>
            </div>
            <div style="padding:4px 0 8px 0">
              <TextField
                value={name}
                placeholder={kind === "audio" ? "New music playlist" : "New video playlist"}
                initial={!!loaded.data && !lists.length}
                onInput={setName}
                onSubmit={() => setTimeout(create, 0)}
              />
            </div>
            <PanelRow label="Create" icon="plus" disabled={!name.trim()} onSelect={create} />
            <div class="t-label c-muted" style="padding:16px 0 6px 16px">
              Your playlists
            </div>
            {!loaded.data && !loaded.error && (
              <>
                <div class="skeleton" style="height:40px;margin:4px 0" />
                <div class="skeleton" style="height:40px;margin:4px 0" />
              </>
            )}
            {loaded.error && !loaded.data && <div class="t-body c-danger" style="padding-left:16px">{problem(loaded.error)}</div>}
            {loaded.data && !lists.length && (
              <div class="t-body c-muted" style="padding-left:16px">
                {kind === "audio" ? "No music playlists yet: name one above." : "No video playlists yet: name one above."}
              </div>
            )}
            {lists.map((pl, i) => (
              <PanelRow key={pl.id} label={pl.title} sub={playlistMeta(pl)} initial={i === 0} onSelect={() => add(pl)} />
            ))}
            {error && <div class="t-body c-danger" style="padding:8px 0 0 16px">{error}</div>}
          </>
        )}
      </SidePanel>
    </div>
  );
}
