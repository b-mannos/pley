import { useEffect, useRef, useState } from "preact/hooks";
import { errorMessage } from "../../api/client";
import type { ItemSummary, Playlist, PlaylistEntry } from "../../api/types";
import { episodeLabel, formatClock, formatDuration, pluralize } from "../../format/format";
import { useUi } from "../context";
import { Pill, PanelRow, Round } from "../kit/buttons";
import { TextField } from "../kit/field";
import { F, G, railOnLeft } from "../kit/focusable";
import { PosterGrid } from "../kit/grid";
import { Icon } from "../kit/icons";
import { PanelHeading, PanelTitle, SidePanel, useScrollToFocus } from "../kit/panel";
import { describeSmartFilter } from "./library-data";
import { ArtTile } from "../kit/tile";
import type { Entry } from "../shell/navigator";
import { ColumnMessage, failedTitle, problem, sentence, useApp, useLoad } from "./common";
import { openPlayer } from "./item-actions";
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

/** A playlist: its title, Play and Shuffle, and its entries, each playable from there. */
export function PlaylistScreen(p: { entry: Entry; id: number }) {
  const ui = useUi();
  const loaded = useLoad((c) => ui.api.playlist(p.id, c), [p.id]);
  const pl = loaded.data;
  const listRef = useRef<HTMLDivElement>(null);
  const shift = useScrollToFocus(listRef, 540 - 150);
  if (!pl) {
    if (loaded.error) return <ColumnMessage top={80} title={failedTitle(loaded.error, "Couldn't load this playlist")} body={problem(loaded.error)} action={{ label: "Retry", icon: "refresh-cw", onSelect: loaded.reload }} />;
    return null;
  }
  const entries = pl.entries;
  const audio = pl.kind === "audio";
  const playFrom = (at: number, shuffle = false) => {
    if (!entries.length) return;
    if (audio) {
      musicPlayer().play(entries.map((e) => e.item), at, shuffle);
      ui.nav.open({ name: "nowPlaying" });
      return;
    }
    const start = shuffle ? Math.floor(Math.random() * entries.length) : at;
    openPlayer(ui, { itemId: entries[start]!.item.id, queue: { kind: "playlist", id: pl.id } });
  };
  return (
    <div class="playlist-page">
      <div class="playlist-header">
        <div class="t-page c-fg one-line">{pl.title}</div>
        <div class="t-label c-muted" style="margin-top:6px">
          {playlistMeta(pl)}
        </div>
        <G kind="row" class="buttons" style="margin-top:16px">
          <Pill label="Play" icon="play" fid="play" initial={entries.length > 0} disabled={!entries.length} onSelect={() => playFrom(0)} />
          <Round label="Shuffle" icon="shuffle" fid="shuffle" offArt disabled={entries.length < 2} onSelect={() => playFrom(0, true)} />
        </G>
      </div>
      <div class="playlist-list-clip">
        <div ref={listRef} style={`transform:translateY(${-shift}px)`}>
          <G kind="col" restore>
            {entries.map((e: PlaylistEntry, i) => (
              <TrackLine
                key={e.entry_id}
                fid={`entry/${e.entry_id}`}
                number={String(i + 1)}
                title={entryTitle(e.item)}
                subtitle={entrySubtitle(e.item)}
                duration={e.item.duration_ms ? formatClock(e.item.duration_ms) : ""}
                onSelect={() => playFrom(i)}
                onActions={() => ui.nav.open({ name: "actions", id: e.item.id })}
              />
            ))}
          </G>
        </div>
      </div>
      {!entries.length && <ColumnMessage top={200} title="This playlist is empty" body="Add titles to it from their pages." takeFocus={false} />}
    </div>
  );
}

function entryTitle(item: ItemSummary): string {
  return item.kind === "episode" ? item.grandparent_title || item.title : item.title;
}

function entrySubtitle(item: ItemSummary): string {
  if (item.kind === "episode") return [episodeLabel(item.parent_index, item.index), item.title].filter(Boolean).join(" · ");
  if (item.kind === "track") return [item.grandparent_title ?? "", item.parent_title ?? ""].filter(Boolean).join(" · ");
  return item.year ? String(item.year) : "";
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
