//! Library browsing: sections, listings with Plex filters, sorting and
//! paging, metadata, hierarchy, hubs, search and artwork.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use axum::Extension;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;

use super::meta::{self, Detail, Extras, Sections, kind_for_number, kinds_of, plex_type, section_type, type_number};
use super::{Node, Page, Params, PlexAdmin, PlexError, PlexInfo, PlexResult, PlexUser};
use crate::auth::AuthUser;
use crate::catalog::{self, ItemRow, LibraryQuery, SortKey};
use crate::events::ServerEvent;
use crate::library::{LibraryRow, list_libraries};
use crate::state::AppState;
use crate::util::now;

/// Libraries the user can see. Per-user library access belongs here.
async fn visible_libraries(state: &AppState, _user: &AuthUser) -> PlexResult<Vec<LibraryRow>> {
    Ok(list_libraries(&state.db).await?)
}

async fn library_for(state: &AppState, user: &AuthUser, id: &str) -> PlexResult<(LibraryRow, Vec<LibraryRow>)> {
    let id: i64 = id.parse().map_err(|_| PlexError::NotFound)?;
    let libraries = visible_libraries(state, user).await?;
    let lib = libraries.iter().find(|l| l.id == id).cloned().ok_or(PlexError::NotFound)?;
    Ok((lib, libraries))
}

fn base_container() -> Node {
    Node::container()
        .attr("allowSync", false)
        .attr("identifier", "com.plexapp.plugins.library")
        .attr("mediaTagPrefix", "/system/bundle/media/flags/")
        .attr("mediaTagVersion", 1)
}

/// A paged list of items.
fn list_container(lib: Option<&LibraryRow>, info: &PlexInfo, nodes: Vec<Node>, total: usize, page: &Page) -> Node {
    let mut mc = base_container().attr("size", nodes.len()).attr("totalSize", total).attr("offset", page.start);
    if let Some(lib) = lib {
        mc = mc
            .attr("librarySectionID", lib.id)
            .attr("librarySectionTitle", &lib.name)
            .attr("librarySectionUUID", info.section_uuid(lib.id))
            .attr("title1", &lib.name);
    }
    mc.extend(nodes);
    mc
}

/// Item elements for rows loaded through the catalog.
async fn item_nodes(state: &AppState, rows: &[ItemRow], sections: &Sections, detail: Detail) -> PlexResult<Vec<Node>> {
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let extras = meta::load_extras(&state.db, &ids).await?;
    let leaves: Vec<i64> = rows.iter().filter(|r| meta::is_leaf(&r.kind)).map(|r| r.id).collect();
    let files =
        if detail.media && !leaves.is_empty() { meta::load_files(&state.db, &leaves).await? } else { HashMap::new() };
    Ok(rows
        .iter()
        .map(|r| {
            let files = files.get(&r.id).map(Vec::as_slice).unwrap_or(&[]);
            meta::item_node(r, extras.get(&r.id), files, sections, detail)
        })
        .collect())
}

fn listing_detail(params: &Params) -> Detail {
    Detail { guids: params.flag("includeGuids"), full: false, media: true }
}

// ------------------------------------------------------------------ sections

pub async fn root(_user: PlexUser) -> Node {
    base_container()
        .attr("size", 3)
        .attr("title1", "Plex Library")
        .attr("title2", "")
        .child(Node::new("Directory").attr("key", "sections").attr("title", "Library Sections"))
        .child(Node::new("Directory").attr("key", "recentlyAdded").attr("title", "Recently Added Content"))
        .child(Node::new("Directory").attr("key", "onDeck").attr("title", "On Deck Content"))
}

fn section_node(state: &AppState, info: &PlexInfo, lib: &LibraryRow) -> Node {
    let (agent, scanner) = match section_type(lib) {
        "movie" => ("tv.plex.agents.movie", "Plex Movie"),
        "show" => ("tv.plex.agents.series", "Plex TV Series"),
        _ => ("tv.plex.agents.music", "Plex Music"),
    };
    let changed = lib.scanned_at.unwrap_or(lib.created_at);
    let mut node = Node::new("Directory")
        .attr("allowSync", false)
        .attr("filters", true)
        .attr("refreshing", state.scanner.is_scanning(lib.id))
        .attr("key", lib.id.to_string())
        .attr("type", section_type(lib))
        .attr("title", &lib.name)
        .attr("agent", agent)
        .attr("scanner", scanner)
        .attr("language", "en-US")
        .attr("uuid", info.section_uuid(lib.id))
        .attr("updatedAt", changed)
        .attr("createdAt", lib.created_at)
        .opt("scannedAt", lib.scanned_at)
        .attr("content", true)
        .attr("directory", true)
        .attr("contentChangedAt", changed)
        .attr("hidden", 0);
    for (i, path) in lib.paths().iter().enumerate() {
        node.push(
            Node::new("Location")
                .attr("id", lib.id * 1000 + i as i64)
                .attr("path", path.to_string_lossy().into_owned()),
        );
    }
    node
}

pub async fn sections(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
) -> PlexResult<Node> {
    let libraries = visible_libraries(&state, &user).await?;
    let mut mc = base_container().attr("size", libraries.len()).attr("title1", "Plex Library");
    for lib in &libraries {
        mc.push(section_node(&state, &info, lib));
    }
    Ok(mc)
}

pub async fn section(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    Path(id): Path<String>,
) -> PlexResult<Node> {
    let (lib, _) = library_for(&state, &user, &id).await?;
    let top = lib.kind().top_level_kind();
    let plural = match top {
        "movie" => "Movies",
        "show" => "Shows",
        _ => "Artists",
    };
    let directory = |key: String, title: &str| Node::new("Directory").attr("key", key).attr("title", title);
    let mut mc = base_container()
        .attr("content", "secondary")
        .attr("librarySectionID", lib.id)
        .attr("librarySectionUUID", info.section_uuid(lib.id))
        .attr("title1", &lib.name)
        .attr("viewGroup", "secondary");
    mc.push(directory("all".into(), &format!("All {plural}")));
    mc.push(directory("recentlyAdded".into(), "Recently Added"));
    if top != "artist" {
        mc.push(directory("onDeck".into(), "On Deck"));
    }
    for (filter, title) in [("genre", "By Genre"), ("year", "By Year"), ("decade", "By Decade")] {
        mc.push(directory(filter.into(), title).attr("secondary", true));
    }
    mc.push(
        directory(format!("search?type={}", type_number(top)), &format!("Search {plural}..."))
            .attr("prompt", format!("Search {plural}"))
            .attr("search", true),
    );
    let size = mc.children().len();
    mc.set("size", size);
    Ok(mc)
}

pub async fn collections(PlexUser(_user): PlexUser) -> Node {
    base_container().attr("size", 0).attr("totalSize", 0).attr("offset", 0)
}

pub async fn refresh_all(
    State(state): State<AppState>,
    PlexAdmin(_admin): PlexAdmin,
    params: Params,
) -> PlexResult<Node> {
    state.scanner.enqueue_all(params.flag("force")).await?;
    Ok(Node::container().attr("size", 0))
}

pub async fn refresh(
    State(state): State<AppState>,
    PlexAdmin(admin): PlexAdmin,
    Path(id): Path<String>,
    params: Params,
) -> PlexResult<Node> {
    let (lib, _) = library_for(&state, &admin, &id).await?;
    state.scanner.enqueue(lib.id, params.flag("force"));
    Ok(Node::container().attr("size", 0))
}

pub async fn refresh_item(
    State(state): State<AppState>,
    PlexAdmin(admin): PlexAdmin,
    Path(id): Path<i64>,
) -> PlexResult<Node> {
    let row = catalog::item(&state.db, admin.id, id).await?.ok_or(PlexError::NotFound)?;
    if state.metadata.enabled() {
        let metadata = state.metadata.clone();
        let events = state.events.clone();
        tokio::spawn(async move {
            match metadata.refresh_item(id).await {
                Ok(()) => events.send(ServerEvent::LibraryChanged { library_id: row.library_id }),
                Err(err) => tracing::warn!("metadata refresh for item {id} failed: {err:#}"),
            }
        });
    }
    Ok(Node::container().attr("size", 0))
}

// ------------------------------------------------------------------ filtering and sorting

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    /// `field=v`: equals (numbers, tags) or contains (strings).
    Is,
    /// `field!=v`
    Not,
    /// `field>>=v`
    Greater,
    /// `field<<=v`
    Less,
    /// `field<=v` (strings)
    Begins,
    /// `field>=v` (strings)
    Ends,
    /// `field==v` (strings)
    Exactly,
    /// `field!==v` (strings)
    NotExactly,
}

/// Which item a filter looks at: the listed one or an ancestor
/// (`artist.id=5` on albums, `show.title=…` on episodes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Own,
    Parent,
    Grandparent,
}

fn level_of(prefix: &str, kind: &str) -> Option<Level> {
    let own = plex_type(kind);
    if prefix == own {
        return Some(Level::Own);
    }
    let (parent, grandparent) = match own {
        "season" => ("show", ""),
        "episode" => ("season", "show"),
        "album" => ("artist", ""),
        "track" => ("album", "artist"),
        _ => ("", ""),
    };
    if prefix == parent {
        Some(Level::Parent)
    } else if !grandparent.is_empty() && prefix == grandparent {
        Some(Level::Grandparent)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
struct Filter {
    level: Level,
    field: String,
    op: Op,
    values: Vec<String>,
}

/// Query parameters that are not filters.
const NOT_FILTERS: &[&str] = &[
    "type",
    "sort",
    "limit",
    "query",
    "includeGuids",
    "includeMeta",
    "includeAdvanced",
    "includeCollections",
    "includeExternalMedia",
    "includeDetails",
    "includeFields",
    "excludeFields",
    "excludeElements",
    "includeMarkers",
    "includeChapters",
    "checkFiles",
    "push",
    "pop",
    "and",
    "or",
    "group",
    "having",
    "sectionId",
    "contentDirectoryID",
    "count",
    "excludeAllLeaves",
    "includeOnDeck",
    "skipRefresh",
    "asyncAugmentMetadata",
];

impl Filter {
    fn parse(params: &Params, kind: &str) -> Vec<Filter> {
        params
            .0
            .iter()
            .filter_map(|(raw_key, raw_value)| {
                if raw_key.len() > 7 && raw_key[..7].eq_ignore_ascii_case("x-plex-") {
                    return None;
                }
                if NOT_FILTERS.contains(&raw_key.as_str()) {
                    return None;
                }
                let mut key = raw_key.as_str();
                let mut level = Level::Own;
                if let Some((prefix, rest)) = key.split_once('.') {
                    // Filters on descendants (`episode.title` on shows) are not supported.
                    level = level_of(prefix, kind)?;
                    key = rest;
                }
                let (field, op) = if let Some(f) = key.strip_suffix(">>") {
                    (f, Op::Greater)
                } else if let Some(f) = key.strip_suffix("<<") {
                    (f, Op::Less)
                } else if let Some(f) = key.strip_suffix('!') {
                    (f, Op::Not)
                } else if let Some(f) = key.strip_suffix('<') {
                    (f, Op::Begins)
                } else if let Some(f) = key.strip_suffix('>') {
                    (f, Op::Ends)
                } else {
                    (key, Op::Is)
                };
                let (op, value) = match (op, raw_value.strip_prefix('=')) {
                    (Op::Is, Some(rest)) => (Op::Exactly, rest),
                    (Op::Not, Some(rest)) => (Op::NotExactly, rest),
                    (op, _) => (op, raw_value.as_str()),
                };
                let values = value.split(',').map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()).collect();
                Some(Filter { level, field: field.to_owned(), op, values })
            })
            .collect()
    }

    fn needs_extras(&self) -> bool {
        matches!(self.field.as_str(), "genre" | "contentRating" | "studio" | "guid")
    }

    /// Unknown fields don't filter anything out.
    fn matches(&self, row: &ItemRow, extras: Option<&Extras>, now: i64) -> bool {
        let ancestor = match self.level {
            Level::Own => None,
            Level::Parent => Some(match self.field.as_str() {
                "id" => Some(self.number(row.parent_id.map(|i| i as f64))),
                "title" => Some(self.text(row.parent_title.as_deref())),
                "index" => Some(self.number(row.parent_idx.map(|i| i as f64))),
                _ => None,
            }),
            Level::Grandparent => Some(match self.field.as_str() {
                "id" => Some(self.number(row.grandparent_id.map(|i| i as f64))),
                "title" => Some(self.text(row.grandparent_title.as_deref())),
                _ => None,
            }),
        };
        if let Some(result) = ancestor {
            return result.unwrap_or(true);
        }
        let watched = match row.kind.as_str() {
            "show" | "season" => {
                let leaves = row.leaf_count.unwrap_or(0);
                leaves > 0 && row.viewed_leaf_count.unwrap_or(0) >= leaves
            }
            _ => row.watched,
        };
        let result = match self.field.as_str() {
            "title" => Some(self.text(Some(&row.title))),
            "studio" => Some(self.text(extras.and_then(|e| e.studio.as_deref()))),
            "year" => Some(self.number(row.year.map(|y| y as f64))),
            "decade" => Some(self.number(row.year.map(|y| (y / 10 * 10) as f64))),
            "index" => Some(self.number(row.idx.map(|i| i as f64))),
            "id" => Some(self.number(Some(row.id as f64))),
            "duration" => Some(self.number(row.duration_ms.map(|d| d as f64))),
            "viewCount" => Some(self.number(Some(meta::view_count(row) as f64))),
            "viewOffset" => Some(self.number(Some(row.position_ms as f64))),
            "rating" | "audienceRating" => Some(self.number(row.rating)),
            "leafCount" => Some(self.number(row.leaf_count.map(|l| l as f64))),
            "genre" => Some(self.tags(extras.map(|e| e.genres.as_slice()).unwrap_or(&[]))),
            "contentRating" => {
                let rating: Vec<String> = extras.and_then(|e| e.content_rating.clone()).into_iter().collect();
                Some(self.tags(&rating))
            }
            "unwatched" | "unwatchedLeaves" => Some(self.boolean(!watched)),
            "watched" => Some(self.boolean(watched)),
            "inProgress" => Some(self.boolean(row.position_ms > 0 && !row.watched)),
            "addedAt" => Some(self.date(Some(row.added_at), now)),
            "updatedAt" => Some(self.date(Some(row.updated_at), now)),
            "lastViewedAt" => Some(self.date(row.last_played_at, now)),
            "originallyAvailableAt" => Some(self.date(row.release_date.as_deref().and_then(date_secs), now)),
            "guid" => {
                let own = meta::guid(&row.kind, row.id);
                let imdb = extras.and_then(|e| e.imdb_id.clone()).map(|i| format!("imdb://{i}"));
                let tmdb = extras.and_then(|e| e.tmdb_id).map(|t| format!("tmdb://{t}"));
                Some(self.values.iter().any(|v| *v == own || Some(v) == imdb.as_ref() || Some(v) == tmdb.as_ref()))
            }
            _ => None,
        };
        result.unwrap_or(true)
    }

    fn text(&self, value: Option<&str>) -> bool {
        let value = value.unwrap_or_default().to_lowercase();
        let any = |f: &dyn Fn(&str) -> bool| self.values.iter().any(|v| f(&v.to_lowercase()));
        match self.op {
            Op::Is => any(&|v| value.contains(v)),
            Op::Not => !any(&|v| value.contains(v)),
            Op::Exactly => any(&|v| value == v),
            Op::NotExactly => !any(&|v| value == v),
            Op::Begins => any(&|v| value.starts_with(v)),
            Op::Ends => any(&|v| value.ends_with(v)),
            Op::Greater | Op::Less => true,
        }
    }

    fn number(&self, value: Option<f64>) -> bool {
        let targets: Vec<f64> = self.values.iter().filter_map(|v| v.parse().ok()).collect();
        let Some(value) = value else { return matches!(self.op, Op::Not | Op::NotExactly) };
        match self.op {
            Op::Is | Op::Exactly => targets.iter().any(|t| (value - t).abs() < 1e-9),
            Op::Not | Op::NotExactly => !targets.iter().any(|t| (value - t).abs() < 1e-9),
            Op::Greater => targets.first().is_some_and(|t| value > *t),
            Op::Less => targets.first().is_some_and(|t| value < *t),
            Op::Begins | Op::Ends => true,
        }
    }

    fn tags(&self, tags: &[String]) -> bool {
        let has = |v: &str| tags.iter().any(|t| t.eq_ignore_ascii_case(v));
        match self.op {
            Op::Not | Op::NotExactly => !self.values.iter().any(|v| has(v)),
            _ => self.values.iter().any(|v| has(v)),
        }
    }

    fn boolean(&self, value: bool) -> bool {
        let wanted = !self.values.first().is_some_and(|v| matches!(v.as_str(), "0" | "false"));
        match self.op {
            Op::Not | Op::NotExactly => value != wanted,
            _ => value == wanted,
        }
    }

    /// Dates are unix seconds or relative (`-30d`: thirty days ago).
    fn date(&self, value: Option<i64>, now: i64) -> bool {
        let Some(value) = value else { return false };
        let Some(target) = self.values.first().and_then(|v| parse_date(v, now)) else { return true };
        match self.op {
            Op::Greater => value > target,
            Op::Less => value < target,
            Op::Not | Op::NotExactly => (value - target).abs() >= 86_400,
            _ => (value - target).abs() < 86_400,
        }
    }
}

fn parse_date(value: &str, now: i64) -> Option<i64> {
    let value = value.trim();
    if let Ok(secs) = value.parse::<i64>() {
        return Some(secs);
    }
    if let Some(secs) = date_secs(value) {
        return Some(secs);
    }
    let relative = value.trim_start_matches('-');
    let (number, unit) = relative.split_at(relative.find(|c: char| !c.is_ascii_digit())?);
    let n: i64 = number.parse().ok()?;
    let unit_secs = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        "w" => 7 * 86_400,
        "mon" => 30 * 86_400,
        "y" => 365 * 86_400,
        _ => return None,
    };
    Some(now - n * unit_secs)
}

/// `YYYY-MM-DD` → unix seconds (UTC midnight).
fn date_secs(date: &str) -> Option<i64> {
    let mut parts = date.get(..10)?.split('-');
    let (y, m, d): (i64, i64, i64) =
        (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortField {
    Title,
    Added,
    Released,
    Year,
    Rating,
    LastViewed,
    ViewCount,
    Duration,
    Index,
    Id,
    Updated,
}

/// `sort=movie.addedAt:desc,titleSort` → fields with direction (descending?).
fn parse_sort(value: Option<&str>) -> Vec<(SortField, bool)> {
    let mut out = Vec::new();
    for spec in value.unwrap_or_default().split(',') {
        let spec = spec.trim();
        if spec.is_empty() {
            continue;
        }
        let (field, dir) = spec.split_once(':').unwrap_or((spec, ""));
        let field = field.rsplit('.').next().unwrap_or(field);
        let field = match field {
            "titleSort" | "title" => SortField::Title,
            "addedAt" => SortField::Added,
            "originallyAvailableAt" => SortField::Released,
            "year" => SortField::Year,
            "rating" | "audienceRating" | "userRating" => SortField::Rating,
            "lastViewedAt" => SortField::LastViewed,
            "viewCount" => SortField::ViewCount,
            "duration" => SortField::Duration,
            "index" => SortField::Index,
            "id" => SortField::Id,
            "updatedAt" => SortField::Updated,
            _ => continue,
        };
        out.push((field, dir == "desc"));
    }
    if out.is_empty() {
        out.push((SortField::Title, false));
    }
    out
}

fn sort_rows(rows: &mut [ItemRow], spec: &[(SortField, bool)]) {
    fn cmp_opt<T: PartialOrd>(a: Option<T>, b: Option<T>, desc: bool) -> Ordering {
        match (a, b) {
            (Some(a), Some(b)) => {
                let o = a.partial_cmp(&b).unwrap_or(Ordering::Equal);
                if desc { o.reverse() } else { o }
            }
            // Missing values last in both directions.
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
    rows.sort_by(|a, b| {
        for &(field, desc) in spec {
            let o = match field {
                SortField::Title => cmp_opt(Some(&a.sort_title), Some(&b.sort_title), desc),
                SortField::Added => cmp_opt(Some(a.added_at), Some(b.added_at), desc),
                SortField::Released => cmp_opt(a.release_date.as_ref(), b.release_date.as_ref(), desc),
                SortField::Year => cmp_opt(a.year, b.year, desc),
                SortField::Rating => cmp_opt(a.rating, b.rating, desc),
                SortField::LastViewed => cmp_opt(a.last_played_at, b.last_played_at, desc),
                SortField::ViewCount => cmp_opt(Some(meta::view_count(a)), Some(meta::view_count(b)), desc),
                SortField::Duration => cmp_opt(a.duration_ms, b.duration_ms, desc),
                SortField::Index => cmp_opt(a.idx, b.idx, desc),
                SortField::Id => cmp_opt(Some(a.id), Some(b.id), desc),
                SortField::Updated => cmp_opt(Some(a.updated_at), Some(b.updated_at), desc),
            };
            if o != Ordering::Equal {
                return o;
            }
        }
        a.sort_title.cmp(&b.sort_title)
    });
}

/// Items of `kind` in `lib` after Plex filters and sorting.
async fn query_items(
    state: &AppState,
    user: &AuthUser,
    lib: &LibraryRow,
    kind: &str,
    params: &Params,
) -> PlexResult<(Vec<ItemRow>, HashMap<i64, Extras>)> {
    let query =
        LibraryQuery { library_id: lib.id, kind, sort: SortKey::Title, descending: false, genre: None, year: None };
    let mut rows = catalog::library_items(&state.db, user.id, &query).await?;
    let filters = Filter::parse(params, kind);
    let mut extras = HashMap::new();
    if filters.iter().any(Filter::needs_extras) {
        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        extras = meta::load_extras(&state.db, &ids).await?;
    }
    if !filters.is_empty() {
        let now = now();
        rows.retain(|r| filters.iter().all(|f| f.matches(r, extras.get(&r.id), now)));
    }
    sort_rows(&mut rows, &parse_sort(params.get("sort")));
    if let Some(limit) = params.parse::<usize>("limit") {
        rows.truncate(limit);
    }
    Ok((rows, extras))
}

/// `/library/sections/{id}/all`
pub async fn all(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    Path(id): Path<String>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let (lib, libraries) = library_for(&state, &user, &id).await?;
    let kind = params.get("type").and_then(kind_for_number).unwrap_or(lib.kind().top_level_kind());
    let rows = if kinds_of(&lib).contains(&kind) {
        query_items(&state, &user, &lib, kind, &params).await?.0
    } else {
        Vec::new()
    };
    let (rows, total) = page.apply(rows);
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    let mut mc = list_container(Some(&lib), &info, nodes, total, &page)
        .attr("title2", format!("All {}", plural(kind)))
        .attr("viewGroup", plex_type(kind));
    if params.flag("includeMeta") {
        mc.push(filter_meta(&lib));
    }
    Ok(mc)
}

/// `/library/sections/{id}/albums` (music sections).
pub async fn section_albums(
    state: State<AppState>,
    user: PlexUser,
    info: Extension<Arc<PlexInfo>>,
    id: Path<String>,
    Params(mut pairs): Params,
    page: Page,
) -> PlexResult<Node> {
    pairs.retain(|(k, _)| k != "type");
    pairs.push(("type".into(), type_number("album").to_string()));
    all(state, user, info, id, Params(pairs), page).await
}

/// `/library/all`: a search across every library.
pub async fn library_all(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let libraries = visible_libraries(&state, &user).await?;
    let wanted = params.get("type").and_then(kind_for_number);
    let mut rows = Vec::new();
    for lib in &libraries {
        let kinds: Vec<&str> = match wanted {
            Some(kind) if kinds_of(lib).contains(&kind) => vec![kind],
            Some(_) => continue,
            None => vec![lib.kind().top_level_kind()],
        };
        for kind in kinds {
            rows.extend(query_items(&state, &user, lib, kind, &params).await?.0);
        }
    }
    sort_rows(&mut rows, &parse_sort(params.get("sort")));
    let (rows, total) = page.apply(rows);
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(list_container(None, &info, nodes, total, &page))
}

fn plural(kind: &str) -> &'static str {
    match kind {
        "movie" => "Movies",
        "show" => "Shows",
        "season" => "Seasons",
        "episode" => "Episodes",
        "artist" => "Artists",
        "album" => "Albums",
        _ => "Tracks",
    }
}

/// `Meta` describing the filters, sorts and fields this API understands
/// (python-plexapi validates searches against it).
fn filter_meta(lib: &LibraryRow) -> Node {
    let mut meta = Node::new("Meta");
    for (i, kind) in kinds_of(lib).iter().enumerate() {
        let n = type_number(kind);
        let mut t = Node::new("Type")
            .attr("key", format!("/library/sections/{}/all?type={n}", lib.id))
            .attr("type", plex_type(kind))
            .attr("title", plural(kind))
            .attr("active", i == 0);
        let mut filters = vec![("genre", "Genre"), ("year", "Year"), ("decade", "Decade")];
        if matches!(*kind, "movie" | "show") {
            filters.push(("contentRating", "Content Rating"));
            filters.push(("studio", "Studio"));
        }
        for (filter, title) in filters {
            t.push(
                Node::new("Filter")
                    .attr("filter", filter)
                    .attr("filterType", if filter == "year" || filter == "decade" { "integer" } else { "string" })
                    .attr("key", format!("/library/sections/{}/{filter}?type={n}", lib.id))
                    .attr("title", title)
                    .attr("type", "filter"),
            );
        }
        if matches!(*kind, "movie" | "episode" | "track" | "show") {
            t.push(
                Node::new("Filter")
                    .attr("filter", "unwatched")
                    .attr("filterType", "boolean")
                    .attr("key", "unwatched")
                    .attr("title", "Unplayed")
                    .attr("type", "filter"),
            );
        }
        for (key, title, dir) in [
            ("titleSort", "Title", "asc"),
            ("addedAt", "Date Added", "desc"),
            ("originallyAvailableAt", "Release Date", "desc"),
            ("year", "Year", "desc"),
            ("rating", "Critic Rating", "desc"),
            ("audienceRating", "Audience Rating", "desc"),
            ("lastViewedAt", "Last Played", "desc"),
            ("viewCount", "Plays", "desc"),
            ("duration", "Duration", "desc"),
        ] {
            t.push(
                Node::new("Sort")
                    .attr("defaultDirection", dir)
                    .attr("descKey", format!("{key}:desc"))
                    .attr("firstCharacterKey", format!("/library/sections/{}/firstCharacter", lib.id))
                    .attr("key", key)
                    .attr("title", title),
            );
        }
        let prefix = if *kind == "movie" { String::new() } else { format!("{}.", plex_type(kind)) };
        for (field, ty, title) in [
            ("title", "string", "Title"),
            ("studio", "string", "Studio"),
            ("year", "integer", "Year"),
            ("decade", "integer", "Decade"),
            ("genre", "tag", "Genre"),
            ("contentRating", "tag", "Content Rating"),
            ("unwatched", "boolean", "Unplayed"),
            ("inProgress", "boolean", "In Progress"),
            ("viewCount", "integer", "Plays"),
            ("addedAt", "date", "Date Added"),
            ("lastViewedAt", "date", "Last Played"),
            ("originallyAvailableAt", "date", "Release Date"),
        ] {
            t.push(Node::new("Field").attr("key", format!("{prefix}{field}")).attr("title", title).attr("type", ty));
        }
        meta.push(t);
    }
    let operators: &[(&str, &[(&str, &str)])] = &[
        ("tag", &[("=", "is"), ("!=", "is not")]),
        ("integer", &[("=", "is"), ("!=", "is not"), (">>=", "is greater than"), ("<<=", "is less than")]),
        (
            "string",
            &[
                ("=", "contains"),
                ("!=", "does not contain"),
                ("==", "is"),
                ("!==", "is not"),
                ("<=", "begins with"),
                (">=", "ends with"),
            ],
        ),
        ("boolean", &[("=", "is true"), ("!=", "is false")]),
        ("date", &[("<<=", "is before"), (">>=", "is after")]),
        ("resolution", &[("=", "is")]),
    ];
    for (ty, ops) in operators {
        let mut field_type = Node::new("FieldType").attr("type", *ty);
        for (key, title) in *ops {
            field_type.push(Node::new("Operator").attr("key", *key).attr("title", *title));
        }
        meta.push(field_type);
    }
    meta
}

/// `/library/sections/{id}/{genre|year|decade|contentRating|studio}`
pub async fn filter_choices(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    Path((id, filter)): Path<(String, String)>,
    params: Params,
) -> PlexResult<Node> {
    let (lib, _) = library_for(&state, &user, &id).await?;
    let kind = params.get("type").and_then(kind_for_number).unwrap_or(lib.kind().top_level_kind());
    let (genres, years) = catalog::library_filters(&state.db, lib.id, kind).await?;
    let values: Vec<String> = match filter.as_str() {
        "genre" => genres,
        "year" => years.iter().map(i64::to_string).collect(),
        "decade" => {
            let mut decades: Vec<i64> = years.iter().map(|y| y / 10 * 10).collect();
            decades.dedup();
            decades.iter().map(i64::to_string).collect()
        }
        "contentRating" | "studio" => {
            let column = if filter == "studio" { "studio" } else { "content_rating" };
            sqlx::query_scalar(&format!(
                "SELECT DISTINCT {column} FROM items WHERE library_id = ? AND kind = ? AND {column} IS NOT NULL
                 ORDER BY {column} COLLATE NOCASE"
            ))
            .bind(lib.id)
            .bind(kind)
            .fetch_all(&state.db.read)
            .await?
        }
        _ => return Err(PlexError::NotFound),
    };
    let mut mc = base_container()
        .attr("size", values.len())
        .attr("librarySectionID", lib.id)
        .attr("librarySectionUUID", info.section_uuid(lib.id))
        .attr("title1", &lib.name)
        .attr("viewGroup", "secondary");
    let n = type_number(kind);
    for value in values {
        mc.push(
            Node::new("Directory")
                .attr("fastKey", format!("/library/sections/{}/all?type={n}&{filter}={}", lib.id, encode(&value)))
                .attr("key", value.clone())
                .attr("title", value)
                .attr("type", filter.clone()),
        );
    }
    Ok(mc)
}

/// Percent-encodes a query value.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

pub async fn first_character(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Path(id): Path<String>,
    params: Params,
) -> PlexResult<Node> {
    let (lib, _) = library_for(&state, &user, &id).await?;
    let kind = params.get("type").and_then(kind_for_number).unwrap_or(lib.kind().top_level_kind());
    let query =
        LibraryQuery { library_id: lib.id, kind, sort: SortKey::Title, descending: false, genre: None, year: None };
    let rows = catalog::library_items(&state.db, user.id, &query).await?;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for row in &rows {
        let first = row.sort_title.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
        let key = if first.chars().all(char::is_alphabetic) && !first.is_empty() { first } else { "#".into() };
        *counts.entry(key).or_default() += 1;
    }
    let mut mc = base_container().attr("size", counts.len()).attr("title1", &lib.name);
    for (letter, size) in counts {
        mc.push(Node::new("Directory").attr("size", size).attr("key", letter.clone()).attr("title", letter));
    }
    Ok(mc)
}

// ------------------------------------------------------------------ metadata

pub async fn metadata(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    Path(ids): Path<String>,
    params: Params,
) -> PlexResult<Node> {
    let ids: Vec<i64> = ids.split(',').filter_map(|i| i.trim().parse().ok()).take(500).collect();
    let rows = catalog::items_by_ids(&state.db, user.id, &ids).await?;
    let libraries = visible_libraries(&state, &user).await?;
    let rows: Vec<ItemRow> = rows.into_iter().filter(|r| libraries.iter().any(|l| l.id == r.library_id)).collect();
    let Some(first) = rows.first() else { return Err(PlexError::NotFound) };
    let lib = libraries.iter().find(|l| l.id == first.library_id);
    let sections = Sections::new(&libraries);
    let mut nodes = item_nodes(&state, &rows, &sections, Detail { guids: true, full: true, media: true }).await?;
    if params.flag("includeOnDeck") {
        for (node, row) in nodes.iter_mut().zip(&rows) {
            if let Some(next) = catalog::on_deck(&state.db, user.id, row).await? {
                if let Some(next_row) = catalog::item(&state.db, user.id, next).await? {
                    let detail = Detail { guids: true, full: false, media: true };
                    let next_node = item_nodes(&state, &[next_row], &sections, detail).await?;
                    let mut on_deck = Node::new("OnDeck");
                    on_deck.extend(next_node);
                    node.push(on_deck);
                }
            }
        }
    }
    let total = nodes.len();
    Ok(list_container(lib, &info, nodes, total, &Page::default()))
}

pub async fn children(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    Path(id): Path<i64>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let parent = catalog::item(&state.db, user.id, id).await?.ok_or(PlexError::NotFound)?;
    let libraries = visible_libraries(&state, &user).await?;
    let lib = libraries.iter().find(|l| l.id == parent.library_id).ok_or(PlexError::NotFound)?;
    let mut rows = catalog::children(&state.db, user.id, id).await?;
    if let Some(index) = params.parse::<i64>("index") {
        rows.retain(|r| r.idx == Some(index));
    }
    let (rows, total) = page.apply(rows);
    let view_group = rows.first().map(|r| plex_type(&r.kind)).unwrap_or("secondary");
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(hierarchy_container(lib, &info, &parent, nodes, total, &page).attr("viewGroup", view_group))
}

fn hierarchy_container(
    lib: &LibraryRow,
    info: &PlexInfo,
    parent: &ItemRow,
    nodes: Vec<Node>,
    total: usize,
    page: &Page,
) -> Node {
    list_container(Some(lib), info, nodes, total, page)
        .attr("key", parent.id.to_string())
        .attr("parentRatingKey", parent.id.to_string())
        .attr("parentTitle", &parent.title)
        .opt("parentIndex", parent.idx)
        .opt("parentYear", parent.year)
        .opt("summary", parent.summary.as_deref())
        .attr("title2", &parent.title)
        .attr("nocache", true)
}

/// Every playable item below a show, season, artist or album.
pub async fn all_leaves(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    Path(id): Path<i64>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let parent = catalog::item(&state.db, user.id, id).await?.ok_or(PlexError::NotFound)?;
    let libraries = visible_libraries(&state, &user).await?;
    let lib = libraries.iter().find(|l| l.id == parent.library_id).ok_or(PlexError::NotFound)?;
    let mut rows = Vec::new();
    if meta::is_leaf(&parent.kind) {
        rows.push(parent.clone());
    } else {
        for child in catalog::children(&state.db, user.id, id).await? {
            if meta::is_leaf(&child.kind) {
                rows.push(child);
            } else {
                rows.extend(catalog::children(&state.db, user.id, child.id).await?);
            }
        }
    }
    let (rows, total) = page.apply(rows);
    let view_group = if matches!(parent.kind.as_str(), "artist" | "album") { "track" } else { "episode" };
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(hierarchy_container(lib, &info, &parent, nodes, total, &page).attr("viewGroup", view_group))
}

// ------------------------------------------------------------------ on deck, recently added, hubs

async fn continue_rows(
    state: &AppState,
    user: &AuthUser,
    libraries: &[LibraryRow],
    library_id: Option<i64>,
    limit: i64,
) -> PlexResult<Vec<ItemRow>> {
    let rows = catalog::continue_watching(&state.db, user.id, library_id, limit).await?;
    Ok(rows.into_iter().filter(|r| libraries.iter().any(|l| l.id == r.library_id)).collect())
}

pub async fn on_deck(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let libraries = visible_libraries(&state, &user).await?;
    let rows = continue_rows(&state, &user, &libraries, None, 50).await?;
    let (rows, total) = page.apply(rows);
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(list_container(None, &info, nodes, total, &page).attr("title1", "On Deck"))
}

pub async fn section_on_deck(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    Path(id): Path<String>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let (lib, libraries) = library_for(&state, &user, &id).await?;
    let rows = continue_rows(&state, &user, &libraries, Some(lib.id), 50).await?;
    let (rows, total) = page.apply(rows);
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(list_container(Some(&lib), &info, nodes, total, &page))
}

pub async fn continue_watching(
    state: State<AppState>,
    user: PlexUser,
    info: Extension<Arc<PlexInfo>>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    on_deck(state, user, info, params, page).await
}

pub async fn section_continue_watching(
    state: State<AppState>,
    user: PlexUser,
    info: Extension<Arc<PlexInfo>>,
    id: Path<String>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    section_on_deck(state, user, info, id, params, page).await
}

async fn recent_rows(state: &AppState, user: &AuthUser, lib: &LibraryRow, limit: i64) -> PlexResult<Vec<ItemRow>> {
    Ok(catalog::recently_added(&state.db, user.id, lib.id, lib.kind().as_str(), limit).await?)
}

pub async fn recently_added(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let libraries = visible_libraries(&state, &user).await?;
    let mut rows = Vec::new();
    for lib in &libraries {
        rows.extend(recent_rows(&state, &user, lib, 50).await?);
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.added_at));
    let (rows, total) = page.apply(rows);
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(list_container(None, &info, nodes, total, &page).attr("title1", "Recently Added"))
}

/// `/hubs/home/recentlyAdded?type=4&sectionID=2`: newest items of one type.
pub async fn home_recently_added(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let libraries = visible_libraries(&state, &user).await?;
    let section = params.parse::<i64>("sectionID");
    let wanted = params.get("type").and_then(kind_for_number);
    let newest = Params(vec![("sort".into(), "addedAt:desc".into()), ("limit".into(), "50".into())]);
    let mut rows = Vec::new();
    for lib in libraries.iter().filter(|l| section.is_none_or(|s| s == l.id)) {
        match wanted {
            Some(kind) if kinds_of(lib).contains(&kind) => {
                rows.extend(query_items(&state, &user, lib, kind, &newest).await?.0);
            }
            Some(_) => {}
            None => rows.extend(recent_rows(&state, &user, lib, 50).await?),
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.added_at));
    let (rows, total) = page.apply(rows);
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(list_container(None, &info, nodes, total, &page).attr("title1", "Recently Added"))
}

pub async fn section_recently_added(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    Path(id): Path<String>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let (lib, libraries) = library_for(&state, &user, &id).await?;
    let rows = recent_rows(&state, &user, &lib, 50).await?;
    let (rows, total) = page.apply(rows);
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(list_container(Some(&lib), &info, nodes, total, &page))
}

fn hub(identifier: String, title: String, hub_type: &str, key: Option<String>, items: Vec<Node>) -> Node {
    let ids: Vec<String> = items
        .iter()
        .filter_map(|n| match n.get("ratingKey") {
            Some(super::xml::Value::Str(id)) => Some(id.clone()),
            _ => None,
        })
        .collect();
    let mut node = Node::new("Hub")
        .attr("hubKey", format!("/library/metadata/{}", ids.join(",")))
        .opt("key", key)
        .attr("title", title)
        .attr("type", hub_type)
        .attr("hubIdentifier", identifier.clone())
        .attr("context", format!("hub.{identifier}"))
        .attr("size", items.len())
        .attr("more", false)
        .attr("style", "shelf")
        .attr("promoted", true);
    node.extend(items);
    node
}

fn hub_type(rows: &[ItemRow]) -> &'static str {
    match rows.first() {
        Some(first) if rows.iter().all(|r| r.kind == first.kind) => plex_type(&first.kind),
        _ => "mixed",
    }
}

pub async fn hubs(State(state): State<AppState>, PlexUser(user): PlexUser, params: Params) -> PlexResult<Node> {
    let count = params.parse::<i64>("count").unwrap_or(12).clamp(1, 100);
    let mut libraries = visible_libraries(&state, &user).await?;
    if let Some(ids) = params.get("contentDirectoryID") {
        let wanted: Vec<i64> = ids.split(',').filter_map(|i| i.trim().parse().ok()).collect();
        libraries.retain(|l| wanted.contains(&l.id));
    }
    let sections = Sections::new(&libraries);
    let detail = listing_detail(&params);
    let mut hubs = Vec::new();
    let rows = continue_rows(&state, &user, &libraries, None, count).await?;
    if !rows.is_empty() {
        let t = hub_type(&rows);
        let items = item_nodes(&state, &rows, &sections, detail).await?;
        hubs.push(hub(
            "home.continue".into(),
            "Continue Watching".into(),
            t,
            Some("/hubs/continueWatching/items".into()),
            items,
        ));
    }
    for lib in &libraries {
        let rows = recent_rows(&state, &user, lib, count).await?;
        if rows.is_empty() {
            continue;
        }
        let area = match section_type(lib) {
            "movie" => "movies",
            "show" => "television",
            _ => "music",
        };
        let t = hub_type(&rows);
        let items = item_nodes(&state, &rows, &sections, detail).await?;
        hubs.push(hub(
            format!("home.{area}.recent"),
            format!("Recently Added in {}", lib.name),
            t,
            Some(format!("/library/sections/{}/recentlyAdded", lib.id)),
            items,
        ));
    }
    if let Some(identifiers) = params.get("identifier") {
        let wanted: Vec<&str> = identifiers.split(',').collect();
        hubs.retain(
            |h| matches!(h.get("hubIdentifier"), Some(super::xml::Value::Str(id)) if wanted.contains(&id.as_str())),
        );
    }
    let mut mc = Node::container()
        .attr("size", hubs.len())
        .attr("allowSync", false)
        .attr("identifier", "com.plexapp.plugins.library");
    mc.extend(hubs);
    Ok(mc)
}

pub async fn section_hubs(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Path(id): Path<String>,
    params: Params,
) -> PlexResult<Node> {
    let (lib, libraries) = library_for(&state, &user, &id).await?;
    let count = params.parse::<i64>("count").unwrap_or(12).clamp(1, 100);
    let sections = Sections::new(&libraries);
    let detail = listing_detail(&params);
    let prefix = match section_type(&lib) {
        "movie" => "movie",
        "show" => "tv",
        _ => "music",
    };
    let top = lib.kind().top_level_kind();
    let mut candidates: Vec<(String, String, Option<String>, Vec<ItemRow>)> = Vec::new();
    if top != "artist" {
        let rows = continue_rows(&state, &user, &libraries, Some(lib.id), count).await?;
        candidates.push((format!("{prefix}.inprogress"), "Continue Watching".into(), None, rows));
    }
    candidates.push((
        format!("{prefix}.recentlyadded"),
        "Recently Added".into(),
        Some(format!("/library/sections/{}/recentlyAdded", lib.id)),
        recent_rows(&state, &user, &lib, count).await?,
    ));
    if top != "artist" {
        let rows = catalog::recently_released(&state.db, user.id, lib.id, top, count).await?;
        candidates.push((format!("{prefix}.recentlyreleased"), "Recently Released".into(), None, rows));
        let rows = catalog::top_rated(&state.db, user.id, lib.id, top, count).await?;
        candidates.push((format!("{prefix}.toprated"), "Top Rated".into(), None, rows));
    }
    let mut mc = Node::container()
        .attr("allowSync", false)
        .attr("librarySectionID", lib.id)
        .attr("librarySectionTitle", &lib.name);
    let mut size = 0;
    for (identifier, title, key, rows) in candidates {
        if rows.is_empty() {
            continue;
        }
        let t = hub_type(&rows);
        let items = item_nodes(&state, &rows, &sections, detail).await?;
        mc.push(hub(identifier, title, t, key, items));
        size += 1;
    }
    mc.set("size", size);
    Ok(mc)
}

// ------------------------------------------------------------------ search

async fn search_rows(
    state: &AppState,
    user: &AuthUser,
    libraries: &[LibraryRow],
    query: &str,
    section: Option<i64>,
) -> PlexResult<Vec<ItemRow>> {
    let rows = catalog::search(&state.db, user.id, query, 200).await?;
    Ok(rows
        .into_iter()
        .filter(|r| libraries.iter().any(|l| l.id == r.library_id) && section.is_none_or(|s| s == r.library_id))
        .collect())
}

pub async fn hub_search(State(state): State<AppState>, PlexUser(user): PlexUser, params: Params) -> PlexResult<Node> {
    let query = params.get("query").unwrap_or_default().trim().to_owned();
    let limit = params.parse::<usize>("limit").unwrap_or(10).clamp(1, 100);
    let libraries = visible_libraries(&state, &user).await?;
    let section = params.parse::<i64>("sectionId");
    let rows = search_rows(&state, &user, &libraries, &query, section).await?;
    let sections = Sections::new(&libraries);
    let mut mc = Node::container().attr("allowSync", false).attr("identifier", "com.plexapp.plugins.library");
    let mut size = 0;
    for kind in ["movie", "show", "season", "episode", "artist", "album", "track"] {
        let group: Vec<ItemRow> = rows.iter().filter(|r| r.kind == kind).take(limit).cloned().collect();
        if group.is_empty() {
            continue;
        }
        let items = item_nodes(&state, &group, &sections, Detail { guids: true, full: false, media: true }).await?;
        mc.push(hub(kind.into(), plural(kind).into(), kind, None, items));
        size += 1;
    }
    mc.set("size", size);
    Ok(mc)
}

pub async fn search(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let libraries = visible_libraries(&state, &user).await?;
    let query = params.get("query").unwrap_or_default().trim().to_owned();
    let mut rows = search_rows(&state, &user, &libraries, &query, params.parse("sectionId")).await?;
    if let Some(kind) = params.get("type").and_then(kind_for_number) {
        rows.retain(|r| r.kind == kind);
    }
    let (rows, total) = page.apply(rows);
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(list_container(None, &info, nodes, total, &page))
}

pub async fn section_search(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Extension(info): Extension<Arc<PlexInfo>>,
    Path(id): Path<String>,
    params: Params,
    page: Page,
) -> PlexResult<Node> {
    let (lib, libraries) = library_for(&state, &user, &id).await?;
    let query = params.get("query").unwrap_or_default().trim().to_owned();
    let mut rows = search_rows(&state, &user, &libraries, &query, Some(lib.id)).await?;
    if let Some(kind) = params.get("type").and_then(kind_for_number) {
        rows.retain(|r| r.kind == kind);
    }
    let (rows, total) = page.apply(rows);
    let nodes = item_nodes(&state, &rows, &Sections::new(&libraries), listing_detail(&params)).await?;
    Ok(list_container(Some(&lib), &info, nodes, total, &page))
}

// ------------------------------------------------------------------ artwork

/// Serves Plex image URLs (`/library/metadata/{id}/thumb/{version}`) with
/// pley's artwork handler: posters, backgrounds and episode stills.
async fn serve_image(
    state: AppState,
    user: AuthUser,
    id: i64,
    image: &str,
    version: Option<String>,
    width: Option<u32>,
    headers: HeaderMap,
) -> PlexResult<Response> {
    let row = catalog::item(&state.db, user.id, id).await?.ok_or(PlexError::NotFound)?;
    let kind = match image {
        "thumb" if row.kind == "episode" => "thumb",
        "thumb" => "poster",
        "art" => "backdrop",
        _ => return Err(PlexError::NotFound),
    };
    let mut query = HashMap::new();
    if let Some(version) = version {
        query.insert("v".to_owned(), version);
    }
    if let Some(width) = width {
        query.insert("w".to_owned(), width.to_string());
    }
    crate::api::items::image(State(state), user, Path((id, kind.to_owned())), Query(query), headers)
        .await
        .map_err(PlexError::from)
}

pub async fn image(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Path((id, image)): Path<(i64, String)>,
    params: Params,
    headers: HeaderMap,
) -> PlexResult<Response> {
    serve_image(state, user, id, &image, None, params.parse("width"), headers).await
}

pub async fn image_version(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    Path((id, image, version)): Path<(i64, String, String)>,
    params: Params,
    headers: HeaderMap,
) -> PlexResult<Response> {
    serve_image(state, user, id, &image, Some(version), params.parse("width"), headers).await
}

/// `/photo/:/transcode?url=/library/metadata/1/thumb/123&width=300&height=450`
pub async fn photo_transcode(
    State(state): State<AppState>,
    PlexUser(user): PlexUser,
    params: Params,
    headers: HeaderMap,
) -> PlexResult<Response> {
    let url = params.get("url").ok_or(PlexError::BadRequest("url is required".into()))?;
    // Absolute URLs pointing back at this server are fine too.
    let path = match url.find("/library/metadata/") {
        Some(start) => &url[start..],
        None => return Err(PlexError::NotFound),
    };
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let mut segments = path.trim_start_matches("/library/metadata/").split('/');
    let id: i64 = segments.next().and_then(|s| s.parse().ok()).ok_or(PlexError::NotFound)?;
    let image = segments.next().unwrap_or("thumb").to_owned();
    let version = segments.next().map(str::to_owned);
    let width = params.parse::<u32>("width").filter(|w| *w > 0);
    serve_image(state, user, id, &image, version, width, headers).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(pairs: &[(&str, &str)]) -> Params {
        Params(pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect())
    }

    #[test]
    fn parses_plex_filters() {
        let filters = Filter::parse(
            &params(&[
                ("type", "1"),
                ("title", "=Exact"),
                ("year>>", "2000"),
                ("genre!", "Horror,Comedy"),
                ("show.title<", "The"),
                ("X-Plex-Token", "secret"),
                ("includeGuids", "1"),
            ]),
            "movie",
        );
        assert_eq!(filters.len(), 3, "{filters:?}");
        assert_eq!((filters[0].field.as_str(), filters[0].op), ("title", Op::Exactly));
        assert_eq!(filters[0].values, vec!["Exact"]);
        assert_eq!((filters[1].field.as_str(), filters[1].op), ("year", Op::Greater));
        assert_eq!((filters[2].op, filters[2].values.len()), (Op::Not, 2));

        let filters = Filter::parse(&params(&[("show.unwatched", "1")]), "show");
        assert_eq!((filters[0].field.as_str(), filters[0].op), ("unwatched", Op::Is));

        // Ancestor filters (python-plexapi's `artist.albums()`), descendants ignored.
        let filters = Filter::parse(&params(&[("artist.id", "16"), ("track.title", "x")]), "album");
        assert_eq!(filters.len(), 1);
        assert_eq!((filters[0].level, filters[0].field.as_str()), (Level::Parent, "id"));
        let filters = Filter::parse(&params(&[("show.id", "3")]), "episode");
        assert_eq!(filters[0].level, Level::Grandparent);
    }

    #[test]
    fn parses_sorts_and_dates() {
        assert_eq!(
            parse_sort(Some("movie.addedAt:desc,titleSort")),
            vec![(SortField::Added, true), (SortField::Title, false)]
        );
        assert_eq!(parse_sort(None), vec![(SortField::Title, false)]);
        assert_eq!(date_secs("1970-01-02"), Some(86_400));
        assert_eq!(date_secs("2000-03-01"), Some(951_868_800));
        assert_eq!(parse_date("-2d", 1_000_000), Some(1_000_000 - 2 * 86_400));
        assert_eq!(parse_date("1700000000", 0), Some(1_700_000_000));
        assert_eq!(encode("Sci-Fi & Fantasy"), "Sci-Fi%20%26%20Fantasy");
    }
}
