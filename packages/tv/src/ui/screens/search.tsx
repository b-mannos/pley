import { useEffect, useRef, useState } from "preact/hooks";
import type { ItemSummary, PersonSummary } from "../../api/types";
import { cardSubtitle, episodeLabel } from "../../format/format";
import { Cancel } from "../../net/transport";
import { useUi } from "../context";
import { Chip, TextButton } from "../kit/buttons";
import { TextField } from "../kit/field";
import { G } from "../kit/focusable";
import { Row, rowHeight, SectionPage, type Section } from "../kit/row";
import { ArtTile, Tile, type TileKind } from "../kit/tile";
import type { Entry } from "../shell/navigator";
import { failedTitle, problem, useApp } from "./common";
import { pageOf } from "./item-actions";
import { RowsSkeleton } from "./library";

const RECENTS = 10;
const ROWS_TOP = 96;

/** The groups results come in, in order, with their headers and tiles. */
const GROUPS: { kind: ItemSummary["kind"] | "person"; title: string; tile: TileKind }[] = [
  { kind: "person", title: "People", tile: "person" },
  { kind: "movie", title: "Movies", tile: "poster" },
  { kind: "show", title: "Shows", tile: "poster" },
  { kind: "season", title: "Seasons", tile: "poster" },
  { kind: "episode", title: "Episodes", tile: "landscape" },
  { kind: "artist", title: "Artists", tile: "square" },
  { kind: "album", title: "Albums", tile: "square" },
  { kind: "track", title: "Tracks", tile: "square" },
];

type Results = { items: ItemSummary[]; people: PersonSummary[] };

/** The recent searches kept on this TV, per server and profile. */
function recentKey(server: string | null, user: number | null): string {
  return `pley.searches.${server ?? ""}.${user ?? 0}`;
}

/** A result's caption under its title. */
function subtitleOf(item: ItemSummary): string {
  if (item.kind === "episode") return [episodeLabel(item.parent_index, item.index), item.title].filter(Boolean).join(" · ");
  if (item.kind === "album") return [item.parent_title ?? "", item.year ? String(item.year) : ""].filter(Boolean).join(" · ");
  if (item.kind === "track") return [item.grandparent_title ?? "", item.parent_title ?? ""].filter(Boolean).join(" · ");
  return cardSubtitle(item);
}

/**
 * Search (TvSearchScreen): the field, typed with the TV's keyboard (and
 * its microphone); results 250 ms after typing pauses, grouped in rows;
 * recent searches while the field is empty.
 */
export function SearchScreen(p: { entry: Entry; query?: string }) {
  const app = useApp();
  const ui = useUi();
  const [text, setText] = useState<string>((p.entry.state.text as string | undefined) ?? p.query ?? "");
  const [results, setResults] = useState<Results | null>((p.entry.state.results as Results | undefined) ?? null);
  const [shownQuery, setShownQuery] = useState<string>((p.entry.state.shownQuery as string | undefined) ?? "");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const key = recentKey(app.server.value, app.me.value?.id ?? null);
  const [recents, setRecents] = useState<string[]>(() => {
    try {
      return JSON.parse(app.store.get(key) ?? "[]") as string[];
    } catch {
      return [];
    }
  });
  const cancel = useRef<Cancel | null>(null);
  const fieldRef = useRef<HTMLElement | null>(null);

  const remember = (q: string) => {
    const t = q.trim();
    if (!t) return;
    const next = [t].concat(recents.filter((r) => r.toLowerCase() !== t.toLowerCase())).slice(0, RECENTS);
    setRecents(next);
    app.store.set(key, JSON.stringify(next));
  };

  // The query 250 ms after typing pauses; a newer one cancels the one asked.
  const query = text.trim();
  useEffect(() => {
    p.entry.state.text = text;
    if (!query) {
      cancel.current?.cancel();
      setResults(null);
      setShownQuery("");
      setLoading(false);
      setError(null);
      return;
    }
    if (query === shownQuery && results) return;
    const t = setTimeout(() => {
      cancel.current?.cancel();
      const c = new Cancel();
      cancel.current = c;
      setLoading(true);
      ui.api.search(query, c).then(
        (r) => {
          if (c.cancelled) return;
          const res = { items: r.items.filter((i) => i.kind !== "extra"), people: r.people ?? [] };
          setResults(res);
          setShownQuery(query);
          setLoading(false);
          setError(null);
          p.entry.state.results = res;
          p.entry.state.shownQuery = query;
        },
        (e) => {
          if (c.cancelled) return;
          // A failure takes the place of what an earlier search found, which no longer matches the field.
          setResults(null);
          setShownQuery("");
          p.entry.state.results = undefined;
          p.entry.state.shownQuery = undefined;
          setLoading(false);
          setError(e);
        },
      );
    }, 250);
    return () => clearTimeout(t);
  }, [query]);

  const focusField = () => {
    const el = fieldRef.current?.querySelector<HTMLElement>("[data-f]");
    return !!el && ui.focus.focus(el);
  };
  const open = (item: ItemSummary) => {
    remember(query);
    ui.nav.open(pageOf(item));
  };

  // The rows: one per group with results.
  const sections: Section[] = [];
  if (results && query) {
    for (const g of GROUPS) {
      if (g.kind === "person") {
        if (!results.people.length) continue;
        sections.push({
          key: "people",
          height: rowHeight("person"),
          anchor: ROWS_TOP,
          render: (onFocus) => (
            <Row
              title={g.title}
              kind="person"
              items={results.people}
              keyOf={(x) => x.id}
              onLeftEdge={ui.toRail}
              onFocusIndex={onFocus}
              tile={(person, _i, place) => (
                <ArtTile
                  kind="person"
                  title={person.name}
                  candidates={person.photo_url ? [person.photo_url] : []}
                  fid={`people/${person.id}`}
                  priority={place.priority}
                  onFocus={place.onFocus}
                  onSelect={() => {
                    remember(query);
                    ui.nav.open({ name: "person", id: person.id });
                  }}
                />
              )}
            />
          ),
        });
        continue;
      }
      const items = results.items.filter((i) => i.kind === g.kind);
      if (!items.length) continue;
      sections.push({
        key: g.kind,
        height: rowHeight(g.tile),
        anchor: ROWS_TOP,
        render: (onFocus) => (
          <Row
            title={g.title}
            kind={g.tile}
            items={items}
            keyOf={(x) => x.id}
            onLeftEdge={ui.toRail}
            onFocusIndex={onFocus}
            tile={(item, _i, place) => (
              <Tile
                item={item}
                kind={g.tile}
                subtitle={subtitleOf(item)}
                fid={`${g.kind}/${item.id}`}
                priority={place.priority}
                onFocus={place.onFocus}
                onSelect={() => open(item)}
                onActions={() => ui.nav.open({ name: "actions", id: item.id })}
              />
            )}
          />
        ),
      });
    }
  }

  const empty = !!results && query && !sections.length;
  return (
    <div class="search-page">
      {sections.length > 0 && (
        <div class="search-rows">
          <SectionPage sections={sections} top={ROWS_TOP} clipAbove={ROWS_TOP - 12} />
        </div>
      )}
      {!results && loading && <RowsSkeleton top={ROWS_TOP} />}
      <G
        kind="row"
        class="search-field"
        elRef={(el) => (fieldRef.current = el)}
        onEdge={(dir) => {
          if (dir === "down") {
            const target = document.querySelector<HTMLElement>(".search-rows [data-f], .search-recents [data-f]");
            return !!target && ui.focus.focus(target);
          }
          return dir === "right";
        }}
      >
        <TextField
          value={text}
          placeholder="Search movies, shows, people…"
          icon="search"
          type="search"
          width={520}
          initial
          onInput={setText}
          onSubmit={(v) => remember(v)}
        />
        {loading && <div class="t-caption c-muted search-busy">Searching…</div>}
      </G>
      {!query && recents.length > 0 && (
        <div class="search-recents">
          <div class="t-section c-fg" style="margin-left:96px">
            Recent searches
          </div>
          <G kind="row" class="recent-chips">
            {recents.map((r) => (
              <Chip
                key={r}
                label={r}
                icon="history"
                maxWidth={320}
                onSelect={() => {
                  setText(r);
                  remember(r);
                  focusField();
                }}
              />
            ))}
            <TextButton
              label="Clear"
              onSelect={() => {
                setRecents([]);
                app.store.remove(key);
                focusField();
              }}
            />
          </G>
        </div>
      )}
      {!query && !recents.length && (
        <div class="search-notice">
          <div class="t-title c-fg">Search your libraries</div>
          <div class="t-body c-muted" style="margin-top:8px">
            Find movies, shows, episodes, people, artists, albums and tracks. Press OK on the field to type, or use your keyboard's microphone.
          </div>
        </div>
      )}
      {query && empty && (
        <div class="search-notice">
          <div class="t-title c-fg">{`No results for “${query}”`}</div>
          <div class="t-body c-muted" style="margin-top:8px">
            Try a different spelling or fewer words.
          </div>
        </div>
      )}
      {query && error && !results ? (
        <div class="search-notice">
          <div class="t-title c-fg">{failedTitle(error, "Search failed")}</div>
          <div class="t-body c-muted" style="margin-top:8px">
            {problem(error)}
          </div>
        </div>
      ) : null}
    </div>
  );
}
