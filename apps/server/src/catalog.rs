//! Read side of the library: item queries, watch state, hubs, search, and the
//! JSON shapes returned by the API (see docs/API.md).

use std::collections::HashMap;

use pley_transcoder_api::{MediaInfo, MediaStream};
use serde::Serialize;
use sqlx::FromRow;

use crate::db::Db;
use crate::library::sidecar::ExternalSub;
use crate::util::now;

/// The requesting user and their restrictions. Item queries bind the user id
/// first and refer to it through `me`, so every placeholder can stay a plain
/// `?` (sqlx numbers unnamed placeholders itself, so they must not be mixed
/// with `?NNN`). Administrators have no restrictions.
macro_rules! me_cte {
    () => {
        "WITH me(uid, all_libs, max_age) AS (
            SELECT id, all_libraries OR is_admin, CASE WHEN is_admin THEN NULL ELSE max_age END FROM users WHERE id = ?)"
    };
}
pub(crate) use me_cte;

/// Whether item `i` (with parent `p` and grandparent `gp`) is visible to the
/// user in `me`: library access and age limit, see [`crate::access`].
pub(crate) const VISIBLE: &str = "((SELECT all_libs FROM me)
       OR i.library_id IN (SELECT library_id FROM user_libraries WHERE user_id = (SELECT uid FROM me)))
  AND ((SELECT max_age FROM me) IS NULL
       OR i.kind IN ('artist', 'album', 'track')
       OR COALESCE(i.rating_age, p.rating_age, gp.rating_age) <= (SELECT max_age FROM me))";

/// Columns of [`ItemRow`]; callers add `WHERE {VISIBLE} AND …`.
/// The `me` CTE that [`VISIBLE`] reads (binds the user id).
pub(crate) const ME_CTE: &str = me_cte!();

const SUMMARY_SELECT: &str = concat!(
    me_cte!(),
    "
SELECT i.id, i.library_id, i.kind, i.title, i.sort_title, i.year, i.idx, i.disc,
       i.parent_id, p.title AS parent_title, p.idx AS parent_idx,
       i.grandparent_id, gp.title AS grandparent_title,
       i.duration_ms, i.rating, i.added_at, i.updated_at, i.summary, i.release_date, i.extra_type,
       i.poster IS NOT NULL AS has_poster, i.backdrop IS NOT NULL AS has_backdrop, i.thumb IS NOT NULL AS has_thumb,
       COALESCE(p.poster IS NOT NULL, 0) AS parent_has_poster, COALESCE(p.backdrop IS NOT NULL, 0) AS parent_has_backdrop,
       p.updated_at AS parent_updated_at,
       COALESCE(gp.poster IS NOT NULL, 0) AS grandparent_has_poster, COALESCE(gp.backdrop IS NOT NULL, 0) AS grandparent_has_backdrop,
       gp.updated_at AS grandparent_updated_at,
       COALESCE(s.position_ms, 0) AS position_ms, COALESCE(s.watched, 0) AS watched,
       COALESCE(s.play_count, 0) AS play_count, s.last_played_at, s.rating AS user_rating,
       i.leaf_count, i.child_count,
       CASE i.kind
         WHEN 'show' THEN (SELECT COUNT(*) FROM items e JOIN user_item_state us ON us.item_id = e.id
                           WHERE e.grandparent_id = i.id AND e.kind = 'episode' AND us.user_id = (SELECT uid FROM me) AND us.watched = 1)
         WHEN 'season' THEN (SELECT COUNT(*) FROM items e JOIN user_item_state us ON us.item_id = e.id
                             WHERE e.parent_id = i.id AND us.user_id = (SELECT uid FROM me) AND us.watched = 1)
         ELSE NULL END AS viewed_leaf_count
FROM items i
LEFT JOIN items p ON p.id = i.parent_id
LEFT JOIN items gp ON gp.id = i.grandparent_id
LEFT JOIN user_item_state s ON s.item_id = i.id AND s.user_id = (SELECT uid FROM me)
"
);

#[derive(Debug, Clone, FromRow)]
pub struct ItemRow {
    pub id: i64,
    pub library_id: i64,
    pub kind: String,
    pub title: String,
    pub sort_title: String,
    pub year: Option<i64>,
    pub idx: Option<i64>,
    pub disc: Option<i64>,
    pub parent_id: Option<i64>,
    pub parent_title: Option<String>,
    pub parent_idx: Option<i64>,
    pub grandparent_id: Option<i64>,
    pub grandparent_title: Option<String>,
    pub duration_ms: Option<i64>,
    pub rating: Option<f64>,
    pub added_at: i64,
    pub updated_at: i64,
    pub summary: Option<String>,
    pub release_date: Option<String>,
    /// Bonus material: `trailer`, `featurette`, … (see `library::walk::ExtraKind`).
    pub extra_type: Option<String>,
    pub has_poster: bool,
    pub has_backdrop: bool,
    pub has_thumb: bool,
    pub parent_has_poster: bool,
    pub parent_has_backdrop: bool,
    pub parent_updated_at: Option<i64>,
    pub grandparent_has_poster: bool,
    pub grandparent_has_backdrop: bool,
    pub grandparent_updated_at: Option<i64>,
    pub position_ms: i64,
    pub watched: bool,
    pub play_count: i64,
    pub last_played_at: Option<i64>,
    pub user_rating: Option<f64>,
    pub leaf_count: Option<i64>,
    pub child_count: Option<i64>,
    pub viewed_leaf_count: Option<i64>,
}

/// Null fields are left out of the JSON: lists of thousands of items are
/// much smaller that way, and clients treat a missing field like `null`.
#[derive(Debug, Clone, Serialize)]
pub struct UserState {
    pub position_ms: i64,
    pub watched: bool,
    pub play_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_played_at: Option<i64>,
    /// The user's star rating, 0-10 (two points per star).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemSummary {
    pub id: i64,
    pub library_id: i64,
    pub kind: String,
    pub title: String,
    pub sort_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grandparent_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grandparent_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
    pub added_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb_url: Option<String>,
    pub user_state: UserState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaf_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewed_leaf_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaFileDto {
    pub id: i64,
    pub path: String,
    pub size: i64,
    pub container: Option<String>,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub streams: Vec<MediaStream>,
    /// Tracks to start with for the requesting user (remembered choices,
    /// see [`crate::streams`]); `None` = the file's default audio / no subtitles.
    pub selected_audio: Option<i64>,
    pub selected_subtitle: Option<i64>,
    pub chapters: Vec<ChapterDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChapterDto {
    pub start_ms: i64,
    pub end_ms: i64,
    pub title: Option<String>,
    /// Picture of the chapter, cut from the seek previews once they exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemDetail {
    #[serde(flatten)]
    pub summary: ItemSummary,
    pub original_title: Option<String>,
    pub tagline: Option<String>,
    pub genres: Vec<String>,
    pub studio: Option<String>,
    pub content_rating: Option<String>,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub files: Vec<MediaFileDto>,
    /// Intro and credits of the files (Skip Intro / Skip Credits).
    pub markers: Vec<crate::markers::Marker>,
    /// Fields edited by hand (kept on refresh), see [`crate::overrides`].
    pub locked_fields: Vec<String>,
    /// Trailers, featurettes and other bonus material (movies and shows).
    pub extras: Vec<ItemSummary>,
    /// Actors in billing order; for episodes, the guest stars.
    pub cast: Vec<crate::people::Credit>,
    /// Creators, directors, writers and producers.
    pub crew: Vec<crate::people::Credit>,
}

pub fn image_url(id: i64, kind: &str, version: i64) -> String {
    format!("/api/items/{id}/images/{kind}?v={version}")
}

impl ItemRow {
    pub fn to_summary(&self) -> ItemSummary {
        let own = |kind: &str| image_url(self.id, kind, self.updated_at);
        let parent = |kind: &str| self.parent_id.map(|p| image_url(p, kind, self.parent_updated_at.unwrap_or(0)));
        let grandparent =
            |kind: &str| self.grandparent_id.map(|g| image_url(g, kind, self.grandparent_updated_at.unwrap_or(0)));

        let poster_url = if self.has_poster {
            Some(own("poster"))
        } else {
            match self.kind.as_str() {
                // Albums fall back to embedded cover art, extracted on first request.
                "album" => Some(own("poster")),
                "track" => parent("poster"),
                "season" if self.parent_has_poster => parent("poster"),
                "episode" if self.grandparent_has_poster => grandparent("poster"),
                _ => None,
            }
        };
        let backdrop_url = if self.has_backdrop {
            Some(own("backdrop"))
        } else if self.parent_has_backdrop {
            parent("backdrop")
        } else if self.grandparent_has_backdrop {
            grandparent("backdrop")
        } else {
            None
        };
        let thumb_url = matches!(self.kind.as_str(), "movie" | "episode" | "extra").then(|| own("thumb"));
        let watched = match self.kind.as_str() {
            "show" | "season" => {
                let leaves = self.leaf_count.unwrap_or(0);
                leaves > 0 && self.viewed_leaf_count.unwrap_or(0) >= leaves
            }
            _ => self.watched,
        };
        ItemSummary {
            id: self.id,
            library_id: self.library_id,
            kind: self.kind.clone(),
            title: self.title.clone(),
            sort_title: self.sort_title.clone(),
            year: self.year,
            index: self.idx,
            parent_id: self.parent_id,
            parent_title: self.parent_title.clone(),
            parent_index: if self.kind == "track" { self.disc } else { self.parent_idx },
            grandparent_id: self.grandparent_id,
            grandparent_title: self.grandparent_title.clone(),
            duration_ms: self.duration_ms,
            rating: self.rating,
            added_at: self.added_at,
            poster_url,
            backdrop_url,
            thumb_url,
            user_state: UserState {
                position_ms: self.position_ms,
                watched,
                play_count: self.play_count,
                last_played_at: self.last_played_at,
                rating: self.user_rating,
            },
            leaf_count: self.leaf_count,
            viewed_leaf_count: self.viewed_leaf_count,
            child_count: self.child_count,
            // Lists only show the synopsis of episodes; it would make a
            // movie library's listing several times larger.
            summary: if self.kind == "episode" { self.summary.clone() } else { None },
            release_date: self.release_date.clone(),
            extra_type: self.extra_type.clone(),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct FileRow {
    pub id: i64,
    pub item_id: i64,
    pub library_id: i64,
    pub path: String,
    pub size: i64,
    pub mtime: i64,
    pub container: Option<String>,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub media_info: Option<String>,
    pub external_subs: String,
    /// See [`crate::store::fingerprint`]; `None` until computed after a scan.
    pub fingerprint: Option<String>,
}

const FILE_SELECT: &str =
    "SELECT id, item_id, library_id, path, size, mtime, container, duration_ms, bitrate, width, height,
    video_codec, audio_codec, media_info, external_subs, fingerprint FROM media_files";

impl FileRow {
    pub fn media_info(&self) -> Option<MediaInfo> {
        self.media_info.as_deref().and_then(|j| serde_json::from_str(j).ok())
    }

    pub fn external_subs(&self) -> Vec<ExternalSub> {
        serde_json::from_str(&self.external_subs).unwrap_or_default()
    }

    /// Embedded streams followed by sidecar subtitles.
    pub fn streams(&self) -> Vec<MediaStream> {
        self.streams_of(self.media_info().map(|m| m.streams).unwrap_or_default())
    }

    fn streams_of(&self, mut streams: Vec<MediaStream>) -> Vec<MediaStream> {
        streams.extend(self.external_subs().iter().map(ExternalSub::to_stream));
        streams
    }

    pub fn to_dto(&self) -> MediaFileDto {
        // Parse the stored media info once.
        let (streams, chapters) = self.media_info().map(|m| (m.streams, m.chapters)).unwrap_or_default();
        MediaFileDto {
            id: self.id,
            path: self.path.clone(),
            size: self.size,
            container: self.container.clone(),
            duration_ms: self.duration_ms,
            bitrate: self.bitrate,
            width: self.width,
            height: self.height,
            video_codec: self.video_codec.clone(),
            audio_codec: self.audio_codec.clone(),
            streams: self.streams_of(streams),
            selected_audio: None,
            selected_subtitle: None,
            chapters: chapters
                .into_iter()
                .map(|c| ChapterDto { start_ms: c.start_ms, end_ms: c.end_ms, title: c.title, thumb_url: None })
                .collect(),
        }
    }
}

/// Whether the user may see the item (library access, age limit), without
/// loading it.
pub async fn is_visible(db: &Db, user_id: i64, item_id: i64) -> sqlx::Result<bool> {
    let sql = format!(
        "{} SELECT EXISTS (SELECT 1 FROM items i LEFT JOIN items p ON p.id = i.parent_id
                           LEFT JOIN items gp ON gp.id = i.grandparent_id WHERE i.id = ? AND {VISIBLE})",
        me_cte!()
    );
    sqlx::query_scalar(&sql).bind(user_id).bind(item_id).fetch_one(&db.read).await
}

pub async fn item(db: &Db, user_id: i64, item_id: i64) -> sqlx::Result<Option<ItemRow>> {
    sqlx::query_as(&format!("{SUMMARY_SELECT} WHERE {VISIBLE} AND i.id = ?"))
        .bind(user_id)
        .bind(item_id)
        .fetch_optional(&db.read)
        .await
}

/// Rows for `ids`, in the order given.
/// Files of movies and episodes, newest first.
pub async fn video_files(db: &Db) -> sqlx::Result<Vec<FileRow>> {
    sqlx::query_as(&format!(
        "{FILE_SELECT} WHERE video_codec IS NOT NULL
         AND item_id IN (SELECT id FROM items WHERE kind IN ('movie', 'episode')) ORDER BY added_at DESC"
    ))
    .fetch_all(&db.read)
    .await
}

pub async fn items_by_ids(db: &Db, user_id: i64, ids: &[i64]) -> sqlx::Result<Vec<ItemRow>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!("{SUMMARY_SELECT} WHERE {VISIBLE} AND i.id IN ({placeholders})");
    let mut query = sqlx::query_as::<_, ItemRow>(&sql).bind(user_id);
    for id in ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(&db.read).await?;
    let mut by_id: HashMap<i64, ItemRow> = rows.into_iter().map(|r| (r.id, r)).collect();
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

/// Seasons, episodes, albums or tracks under an item (not its extras).
pub async fn children(db: &Db, user_id: i64, parent_id: i64) -> sqlx::Result<Vec<ItemRow>> {
    sqlx::query_as(&format!(
        "{SUMMARY_SELECT} WHERE {VISIBLE} AND i.parent_id = ? AND i.kind != 'extra'
         ORDER BY CASE WHEN i.kind = 'season' AND i.idx = 0 THEN 1 ELSE 0 END, i.disc, i.idx, i.sort_title"
    ))
    .bind(user_id)
    .bind(parent_id)
    .fetch_all(&db.read)
    .await
}

/// Bonus material of a movie or show: trailers first, then by kind and title.
pub async fn extras(db: &Db, user_id: i64, parent_id: i64) -> sqlx::Result<Vec<ItemRow>> {
    sqlx::query_as(&format!(
        "{SUMMARY_SELECT} WHERE {VISIBLE} AND i.parent_id = ? AND i.kind = 'extra'
         ORDER BY i.extra_type != 'trailer', i.extra_type, i.sort_title"
    ))
    .bind(user_id)
    .bind(parent_id)
    .fetch_all(&db.read)
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Title,
    Year,
    Added,
    Rating,
    Release,
    Duration,
    LastPlayed,
    UserRating,
}

impl SortKey {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "title" => SortKey::Title,
            "year" => SortKey::Year,
            "added" => SortKey::Added,
            "rating" => SortKey::Rating,
            "release" => SortKey::Release,
            "duration" => SortKey::Duration,
            "last_played" => SortKey::LastPlayed,
            "user_rating" => SortKey::UserRating,
            _ => return None,
        })
    }

    /// The sort expression for items of `kind`, and whether it can be NULL.
    fn column(self, kind: &str) -> (&'static str, bool) {
        let show = kind == "show";
        match self {
            SortKey::Title => ("i.sort_title", false),
            SortKey::Year => ("i.year", true),
            // Shows are "added" when their newest episode was.
            SortKey::Added if show => {
                ("COALESCE((SELECT MAX(e.added_at) FROM items e WHERE e.grandparent_id = i.id), i.added_at)", false)
            }
            SortKey::Added => ("i.added_at", false),
            SortKey::Rating => ("i.rating", true),
            SortKey::Release => ("i.release_date", true),
            SortKey::Duration => ("i.duration_ms", true),
            SortKey::LastPlayed if show => (
                "COALESCE(s.last_played_at, (SELECT MAX(us.last_played_at) FROM items e JOIN user_item_state us ON us.item_id = e.id AND us.user_id = (SELECT uid FROM me) WHERE e.grandparent_id = i.id))",
                true,
            ),
            SortKey::LastPlayed => ("s.last_played_at", true),
            SortKey::UserRating => ("s.rating", true),
        }
    }

    /// `ORDER BY` clause: NULLs last in both directions, title as tiebreaker.
    /// Plain non-null columns stay bare so SQLite can walk an index instead of
    /// sorting the whole library.
    fn order_by(self, kind: &str, descending: bool) -> String {
        let dir = if descending { "DESC" } else { "ASC" };
        match self.column(kind) {
            ("i.sort_title", _) => format!("i.sort_title {dir}"),
            (col, false) => format!("{col} {dir}, i.sort_title ASC"),
            (col, true) => format!("({col}) IS NULL, {col} {dir}, i.sort_title ASC"),
        }
    }
}

pub struct LibraryQuery<'a> {
    pub library_id: i64,
    pub kind: &'a str,
    pub sort: SortKey,
    pub descending: bool,
    pub genre: Option<&'a str>,
    pub year: Option<i64>,
    /// Movies and episodes: watched flag; shows: every episode watched.
    pub watched: Option<bool>,
    pub offset: i64,
    pub limit: Option<i64>,
}

impl<'a> LibraryQuery<'a> {
    /// Every item of `kind` in the library, by title.
    pub fn all(library_id: i64, kind: &'a str) -> Self {
        LibraryQuery {
            library_id,
            kind,
            sort: SortKey::Title,
            descending: false,
            genre: None,
            year: None,
            watched: None,
            offset: 0,
            limit: None,
        }
    }
}

/// One page of a library (`limit` items from `offset`) and the total count.
/// Filtering, sorting and paging all happen in SQL.
pub async fn library_items(db: &Db, user_id: i64, q: &LibraryQuery<'_>) -> sqlx::Result<(Vec<ItemRow>, i64)> {
    let mut filter = String::from(" AND i.library_id = ? AND i.kind = ?");
    if q.genre.is_some() {
        filter.push_str(" AND EXISTS (SELECT 1 FROM json_each(i.genres) g WHERE g.value = ?)");
    }
    if q.year.is_some() {
        filter.push_str(" AND i.year = ?");
    }
    if q.watched.is_some() {
        filter.push_str(
            " AND (CASE WHEN i.kind = 'show' THEN COALESCE(i.leaf_count, 0) > 0 AND
                     (SELECT COUNT(*) FROM items e JOIN user_item_state us ON us.item_id = e.id
                      WHERE e.grandparent_id = i.id AND e.kind = 'episode' AND us.user_id = (SELECT uid FROM me)
                        AND us.watched = 1) >= i.leaf_count
                   ELSE COALESCE(s.watched, 0) = 1 END) = ?",
        );
    }
    let limit = q.limit.unwrap_or(i64::MAX).max(0);
    let sql = format!(
        "{SUMMARY_SELECT} WHERE {VISIBLE}{filter} ORDER BY {} LIMIT ? OFFSET ?",
        q.sort.order_by(q.kind, q.descending)
    );
    macro_rules! bind_filter {
        ($query:expr) => {{
            let mut query = $query.bind(user_id).bind(q.library_id).bind(q.kind);
            if let Some(genre) = q.genre {
                query = query.bind(genre);
            }
            if let Some(year) = q.year {
                query = query.bind(year);
            }
            if let Some(watched) = q.watched {
                query = query.bind(watched);
            }
            query
        }};
    }
    let rows: Vec<ItemRow> =
        bind_filter!(sqlx::query_as::<_, ItemRow>(&sql)).bind(limit).bind(q.offset.max(0)).fetch_all(&db.read).await?;
    // Counting is only needed when the page may not hold everything.
    let total = if q.offset == 0 && (rows.len() as i64) < limit {
        rows.len() as i64
    } else {
        let count = format!(
            "{} SELECT COUNT(*) FROM items i LEFT JOIN items p ON p.id = i.parent_id
                 LEFT JOIN items gp ON gp.id = i.grandparent_id
                 LEFT JOIN user_item_state s ON s.item_id = i.id AND s.user_id = (SELECT uid FROM me)
             WHERE {VISIBLE}{filter}",
            me_cte!()
        );
        bind_filter!(sqlx::query_scalar::<_, i64>(&count)).fetch_one(&db.read).await?
    };
    Ok((rows, total))
}

pub async fn library_filters(db: &Db, library_id: i64, kind: &str) -> sqlx::Result<(Vec<String>, Vec<i64>)> {
    let genres: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT g.value FROM items i, json_each(i.genres) g
         WHERE i.library_id = ? AND i.kind = ? AND i.genres IS NOT NULL ORDER BY g.value COLLATE NOCASE",
    )
    .bind(library_id)
    .bind(kind)
    .fetch_all(&db.read)
    .await?;
    let years: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT year FROM items WHERE library_id = ? AND kind = ? AND year IS NOT NULL ORDER BY year DESC",
    )
    .bind(library_id)
    .bind(kind)
    .fetch_all(&db.read)
    .await?;
    Ok((genres, years))
}

pub async fn files_for_item(db: &Db, item_id: i64) -> sqlx::Result<Vec<FileRow>> {
    sqlx::query_as(&format!("{FILE_SELECT} WHERE item_id = ? ORDER BY COALESCE(height, 0) DESC, id"))
        .bind(item_id)
        .fetch_all(&db.read)
        .await
}

pub async fn file(db: &Db, file_id: i64) -> sqlx::Result<Option<FileRow>> {
    sqlx::query_as(&format!("{FILE_SELECT} WHERE id = ?")).bind(file_id).fetch_optional(&db.read).await
}

#[derive(FromRow)]
struct DetailRow {
    original_title: Option<String>,
    tagline: Option<String>,
    genres: Option<String>,
    studio: Option<String>,
    content_rating: Option<String>,
    tmdb_id: Option<i64>,
    imdb_id: Option<String>,
}

pub async fn item_detail(db: &Db, row: ItemRow) -> sqlx::Result<ItemDetail> {
    let detail: DetailRow = sqlx::query_as(
        "SELECT original_title, tagline, genres, studio, content_rating, tmdb_id, imdb_id FROM items WHERE id = ?",
    )
    .bind(row.id)
    .fetch_one(&db.read)
    .await?;
    let files = files_for_item(db, row.id).await?;
    let markers = crate::markers::for_item(db, row.id).await?;
    // Seasons have the cast and crew of their show.
    let credits_of = if row.kind == "season" { row.parent_id.unwrap_or(row.id) } else { row.id };
    let (cast, crew) =
        crate::people::for_item(db, credits_of).await?.into_iter().partition(|c| c.role == crate::people::Role::Actor);
    let mut summary = row.to_summary();
    summary.summary = row.summary;
    Ok(ItemDetail {
        summary,
        original_title: detail.original_title,
        tagline: detail.tagline,
        genres: detail.genres.and_then(|g| serde_json::from_str(&g).ok()).unwrap_or_default(),
        studio: detail.studio,
        content_rating: detail.content_rating,
        tmdb_id: detail.tmdb_id,
        imdb_id: detail.imdb_id,
        files: files.iter().map(FileRow::to_dto).collect(),
        markers,
        locked_fields: Vec::new(),
        extras: Vec::new(),
        cast,
        crew,
    })
}

// ------------------------------------------------------------------ watch state

pub const WATCHED_THRESHOLD: f64 = 0.9;
const MIN_RESUME_MS: i64 = 15_000;

/// Record playback progress. Crossing 90 % marks the item watched and, the
/// first time for this play, counts it and adds it to the watch history from
/// `device`; returns whether that happened.
pub async fn record_progress(
    db: &Db,
    user_id: i64,
    item_id: i64,
    position_ms: i64,
    duration_ms: i64,
    device: Option<&str>,
) -> sqlx::Result<bool> {
    let now = now();
    let position_ms = position_ms.max(0);
    if duration_ms > 0 && position_ms as f64 >= duration_ms as f64 * WATCHED_THRESHOLD {
        let mut tx = db.write.begin().await?;
        // Reports keep coming after the threshold: a play counts once, until
        // the item is resumed or played again (which sets a position).
        let counted: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM user_item_state
                            WHERE user_id = ? AND item_id = ? AND watched = 1 AND position_ms = 0)",
        )
        .bind(user_id)
        .bind(item_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO user_item_state (user_id, item_id, position_ms, watched, play_count, last_played_at, updated_at)
             VALUES (?, ?, 0, 1, 1, ?, ?)
             ON CONFLICT(user_id, item_id) DO UPDATE SET
                play_count = CASE WHEN user_item_state.position_ms > 0 OR user_item_state.watched = 0
                                  THEN user_item_state.play_count + 1 ELSE user_item_state.play_count END,
                watched = 1, position_ms = 0, last_played_at = excluded.last_played_at, updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(item_id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        if !counted {
            sqlx::query("INSERT INTO play_history (user_id, item_id, viewed_at, device) VALUES (?, ?, ?, ?)")
                .bind(user_id)
                .bind(item_id)
                .bind(now)
                .bind(device)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        return Ok(!counted);
    }
    let stored = if position_ms >= MIN_RESUME_MS { position_ms } else { 0 };
    sqlx::query(
        "INSERT INTO user_item_state (user_id, item_id, position_ms, watched, play_count, last_played_at, updated_at)
         VALUES (?, ?, ?, 0, 0, ?, ?)
         ON CONFLICT(user_id, item_id) DO UPDATE SET
            position_ms = excluded.position_ms, last_played_at = excluded.last_played_at, updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(item_id)
    .bind(stored)
    .bind(now)
    .bind(now)
    .execute(&db.write)
    .await?;
    Ok(false)
}

/// Set (0-10) or clear the user's star rating of an item.
pub async fn set_rating(db: &Db, user_id: i64, item_id: i64, rating: Option<f64>) -> sqlx::Result<()> {
    let now = now();
    sqlx::query(
        "INSERT INTO user_item_state (user_id, item_id, rating, rated_at, updated_at) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, item_id) DO UPDATE SET
            rating = excluded.rating, rated_at = excluded.rated_at, updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(item_id)
    .bind(rating)
    .bind(rating.map(|_| now))
    .bind(now)
    .execute(&db.write)
    .await?;
    Ok(())
}

/// A rating in Plex's 0-10 range, rounded to half stars; `None` for anything else.
pub fn valid_rating(value: f64) -> Option<f64> {
    (value.is_finite() && (0.0..=10.0).contains(&value)).then(|| value.round())
}

/// Leaf items covered by `item` (itself for movies/episodes/tracks).
fn leaves_clause(kind: &str) -> &'static str {
    match kind {
        "show" => "SELECT id FROM items WHERE grandparent_id = ?3 AND kind = 'episode'",
        "season" | "album" => "SELECT id FROM items WHERE parent_id = ?3",
        "artist" => "SELECT id FROM items WHERE grandparent_id = ?3 AND kind = 'track'",
        _ => "SELECT id FROM items WHERE id = ?3",
    }
}

pub async fn set_watched(db: &Db, user_id: i64, item_id: i64, kind: &str, watched: bool) -> sqlx::Result<()> {
    let leaves = leaves_clause(kind);
    let now = now();
    if watched {
        sqlx::query(&format!(
            "INSERT INTO user_item_state (user_id, item_id, position_ms, watched, play_count, last_played_at, updated_at)
             SELECT ?1, id, 0, 1, 1, ?2, ?2 FROM items WHERE id IN ({leaves})
             ON CONFLICT(user_id, item_id) DO UPDATE SET
                play_count = CASE WHEN user_item_state.watched = 0 THEN user_item_state.play_count + 1 ELSE user_item_state.play_count END,
                watched = 1, position_ms = 0, last_played_at = excluded.last_played_at, updated_at = excluded.updated_at"
        ))
        .bind(user_id)
        .bind(now)
        .bind(item_id)
        .execute(&db.write)
        .await?;
    } else {
        sqlx::query(&format!(
            "UPDATE user_item_state SET watched = 0, position_ms = 0, updated_at = ?2
             WHERE user_id = ?1 AND item_id IN ({leaves})"
        ))
        .bind(user_id)
        .bind(now)
        .bind(item_id)
        .execute(&db.write)
        .await?;
    }
    Ok(())
}

// ------------------------------------------------------------------ hubs

pub async fn in_progress(db: &Db, user_id: i64, library_id: Option<i64>, limit: i64) -> sqlx::Result<Vec<ItemRow>> {
    let scope = if library_id.is_some() { "AND i.library_id = ?" } else { "" };
    let sql = format!(
        "{SUMMARY_SELECT} WHERE {VISIBLE} AND s.position_ms > 0 AND i.kind IN ('movie', 'episode') {scope}
         ORDER BY s.last_played_at DESC LIMIT ?"
    );
    let mut q = sqlx::query_as::<_, ItemRow>(&sql).bind(user_id);
    if let Some(lib) = library_id {
        q = q.bind(lib);
    }
    q.bind(limit).fetch_all(&db.read).await
}

#[derive(FromRow)]
struct EpisodeState {
    id: i64,
    watched: bool,
    position_ms: i64,
    last_played_at: Option<i64>,
}

/// Episodes of a show in viewing order (specials last) with the user's state.
async fn show_episodes(db: &Db, user_id: i64, show_id: i64) -> sqlx::Result<Vec<EpisodeState>> {
    sqlx::query_as(
        "SELECT e.id, COALESCE(s.watched, 0) AS watched, COALESCE(s.position_ms, 0) AS position_ms, s.last_played_at
         FROM items e JOIN items se ON se.id = e.parent_id
         LEFT JOIN user_item_state s ON s.item_id = e.id AND s.user_id = ?
         WHERE e.grandparent_id = ? AND e.kind = 'episode'
         ORDER BY se.idx = 0, se.idx, e.idx",
    )
    .bind(user_id)
    .bind(show_id)
    .fetch_all(&db.read)
    .await
}

/// The episode after the most recently watched one, if unwatched and not started.
fn next_after_last_watched(episodes: &[EpisodeState]) -> Option<i64> {
    let last = episodes
        .iter()
        .enumerate()
        .filter(|(_, e)| e.watched && e.last_played_at.is_some())
        .max_by_key(|(_, e)| e.last_played_at)
        .map(|(i, _)| i)?;
    episodes[last + 1..].iter().find(|e| !e.watched).filter(|e| e.position_ms == 0).map(|e| e.id)
}

/// "Next up" episodes for shows with recent activity: (episode id, activity time).
pub async fn next_up(db: &Db, user_id: i64, library_id: Option<i64>, limit: i64) -> sqlx::Result<Vec<(i64, i64)>> {
    let scope = if library_id.is_some() { "AND e.library_id = ?" } else { "" };
    let sql = format!(
        "SELECT e.grandparent_id, MAX(s.last_played_at) FROM user_item_state s JOIN items e ON e.id = s.item_id
         WHERE s.user_id = ? AND e.kind = 'episode' AND s.last_played_at IS NOT NULL {scope}
         GROUP BY e.grandparent_id ORDER BY 2 DESC LIMIT ?"
    );
    let mut q = sqlx::query_as::<_, (i64, i64)>(&sql).bind(user_id);
    if let Some(lib) = library_id {
        q = q.bind(lib);
    }
    let shows = q.bind(limit * 2).fetch_all(&db.read).await?;
    let mut out = Vec::new();
    for (show_id, activity) in shows {
        let episodes = show_episodes(db, user_id, show_id).await?;
        if let Some(next) = next_after_last_watched(&episodes) {
            out.push((next, activity));
        }
        if out.len() as i64 >= limit {
            break;
        }
    }
    Ok(out)
}

/// Continue watching = in-progress items + next-up episodes, newest activity first.
pub async fn continue_watching(
    db: &Db,
    user_id: i64,
    library_id: Option<i64>,
    limit: i64,
) -> sqlx::Result<Vec<ItemRow>> {
    let mut entries: Vec<(i64, i64)> = in_progress(db, user_id, library_id, limit)
        .await?
        .into_iter()
        .map(|r| (r.id, r.last_played_at.unwrap_or(0)))
        .collect();
    for (id, at) in next_up(db, user_id, library_id, limit).await? {
        if !entries.iter().any(|(e, _)| *e == id) {
            entries.push((id, at));
        }
    }
    entries.sort_by_key(|e| std::cmp::Reverse(e.1));
    entries.truncate(limit as usize);
    let ids: Vec<i64> = entries.into_iter().map(|(id, _)| id).collect();
    items_by_ids(db, user_id, &ids).await
}

/// Episode to start when pressing Play on a show or season.
pub async fn on_deck(db: &Db, user_id: i64, item: &ItemRow) -> sqlx::Result<Option<i64>> {
    let (show_id, season_id) = match item.kind.as_str() {
        "show" => (item.id, None),
        "season" => (item.parent_id.unwrap_or_default(), Some(item.id)),
        _ => return Ok(None),
    };
    let mut episodes = show_episodes(db, user_id, show_id).await?;
    if let Some(season) = season_id {
        let in_season: Vec<i64> =
            sqlx::query_scalar("SELECT id FROM items WHERE parent_id = ?").bind(season).fetch_all(&db.read).await?;
        episodes.retain(|e| in_season.contains(&e.id));
    }
    let in_progress =
        episodes.iter().filter(|e| e.position_ms > 0 && !e.watched).max_by_key(|e| e.last_played_at).map(|e| e.id);
    Ok(in_progress
        .or_else(|| next_after_last_watched(&episodes))
        .or_else(|| episodes.iter().find(|e| !e.watched).map(|e| e.id))
        .or_else(|| episodes.first().map(|e| e.id)))
}

/// The episode/track following `item` (crossing season/disc boundaries).
pub async fn next_item(db: &Db, item: &ItemRow) -> sqlx::Result<Option<i64>> {
    match item.kind.as_str() {
        "episode" => {
            let ids: Vec<i64> = sqlx::query_scalar(
                "SELECT e.id FROM items e JOIN items se ON se.id = e.parent_id
                 WHERE e.grandparent_id = ? AND e.kind = 'episode' ORDER BY se.idx = 0, se.idx, e.idx",
            )
            .bind(item.grandparent_id)
            .fetch_all(&db.read)
            .await?;
            Ok(ids.iter().position(|id| *id == item.id).and_then(|i| ids.get(i + 1)).copied())
        }
        "track" => {
            let ids: Vec<i64> =
                sqlx::query_scalar("SELECT id FROM items WHERE parent_id = ? ORDER BY disc, idx, sort_title")
                    .bind(item.parent_id)
                    .fetch_all(&db.read)
                    .await?;
            Ok(ids.iter().position(|id| *id == item.id).and_then(|i| ids.get(i + 1)).copied())
        }
        _ => Ok(None),
    }
}

pub async fn recently_added(
    db: &Db,
    user_id: i64,
    library_id: i64,
    library_kind: &str,
    limit: i64,
) -> sqlx::Result<Vec<ItemRow>> {
    match library_kind {
        "shows" => {
            let ids: Vec<i64> = sqlx::query_scalar(
                "SELECT grandparent_id FROM items WHERE library_id = ? AND kind = 'episode' AND grandparent_id IS NOT NULL
                 GROUP BY grandparent_id ORDER BY MAX(added_at) DESC LIMIT ?",
            )
            .bind(library_id)
            .bind(limit)
            .fetch_all(&db.read)
            .await?;
            items_by_ids(db, user_id, &ids).await
        }
        kind => {
            let item_kind = if kind == "music" { "album" } else { "movie" };
            sqlx::query_as(&format!(
                "{SUMMARY_SELECT} WHERE {VISIBLE} AND i.library_id = ? AND i.kind = ? ORDER BY i.added_at DESC, i.id DESC LIMIT ?"
            ))
            .bind(user_id)
            .bind(library_id)
            .bind(item_kind)
            .bind(limit)
            .fetch_all(&db.read)
            .await
        }
    }
}

pub async fn recently_released(
    db: &Db,
    user_id: i64,
    library_id: i64,
    kind: &str,
    limit: i64,
) -> sqlx::Result<Vec<ItemRow>> {
    sqlx::query_as(&format!(
        "{SUMMARY_SELECT} WHERE {VISIBLE} AND i.library_id = ? AND i.kind = ? AND i.release_date IS NOT NULL
         ORDER BY i.release_date DESC LIMIT ?"
    ))
    .bind(user_id)
    .bind(library_id)
    .bind(kind)
    .bind(limit)
    .fetch_all(&db.read)
    .await
}

pub async fn top_rated(db: &Db, user_id: i64, library_id: i64, kind: &str, limit: i64) -> sqlx::Result<Vec<ItemRow>> {
    sqlx::query_as(&format!(
        "{SUMMARY_SELECT} WHERE {VISIBLE} AND i.library_id = ? AND i.kind = ? AND i.rating IS NOT NULL ORDER BY i.rating DESC, i.sort_title LIMIT ?"
    ))
    .bind(user_id)
    .bind(library_id)
    .bind(kind)
    .bind(limit)
    .fetch_all(&db.read)
    .await
}

// ------------------------------------------------------------------ search

/// `"matr reload"` → `"matr"* "reload"*` (every word, prefix match).
pub fn fts_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .take(8)
        .map(|t| format!("\"{}\"*", t.to_lowercase()))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

pub async fn search(db: &Db, user_id: i64, input: &str, limit: i64) -> sqlx::Result<Vec<ItemRow>> {
    let Some(fts) = fts_query(input) else { return Ok(Vec::new()) };
    let ids: Vec<i64> =
        match sqlx::query_scalar("SELECT rowid FROM items_fts WHERE items_fts MATCH ? ORDER BY rank LIMIT 300")
            .bind(&fts)
            .fetch_all(&db.read)
            .await
        {
            Ok(ids) => ids,
            Err(err) => {
                tracing::debug!("fts query failed ({err}), falling back to LIKE");
                sqlx::query_scalar("SELECT id FROM items WHERE title LIKE ? ESCAPE '\\' LIMIT 300")
                    .bind(format!("%{}%", input.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")))
                    .fetch_all(&db.read)
                    .await?
            }
        };
    let mut rows = items_by_ids(db, user_id, &ids).await?;
    // Bonus material is found through its movie or show.
    rows.retain(|r| r.kind != "extra");
    // Keep relevance order but surface top-level items first.
    let priority = |kind: &str| match kind {
        "movie" | "show" | "artist" => 0,
        "album" | "season" => 1,
        _ => 2,
    };
    rows.sort_by_key(|r| priority(&r.kind));
    rows.truncate(limit as usize);
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_fts_queries() {
        assert_eq!(fts_query("Matrix Reload").as_deref(), Some("\"matrix\"* \"reload\"*"));
        assert_eq!(fts_query("  ").as_deref(), None);
        assert_eq!(fts_query("l'été").as_deref(), Some("\"l\"* \"été\"*"));
    }

    fn ep(id: i64, watched: bool, position_ms: i64, last: Option<i64>) -> EpisodeState {
        EpisodeState { id, watched, position_ms, last_played_at: last }
    }

    #[test]
    fn next_up_follows_last_watched() {
        let eps =
            vec![ep(1, true, 0, Some(10)), ep(2, true, 0, Some(20)), ep(3, false, 0, None), ep(4, false, 0, None)];
        assert_eq!(next_after_last_watched(&eps), Some(3));
        // Rewatched an early episode most recently → next after it is already watched → skip to first unwatched after it.
        let eps = vec![ep(1, true, 0, Some(30)), ep(2, true, 0, Some(20)), ep(3, false, 0, None)];
        assert_eq!(next_after_last_watched(&eps), Some(3));
        // Nothing watched → no next-up.
        assert_eq!(next_after_last_watched(&[ep(1, false, 0, None)]), None);
        // Next one in progress → it's in "continue watching" already.
        let eps = vec![ep(1, true, 0, Some(10)), ep(2, false, 5000, Some(11))];
        assert_eq!(next_after_last_watched(&eps), None);
    }
}
