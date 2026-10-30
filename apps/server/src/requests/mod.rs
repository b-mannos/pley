//! Requests: people ask for movies and shows the libraries don't have,
//! found on TMDB ([`crate::discover`]). Administrators approve or decline
//! them (their own, and those of accounts allowed to, are approved as
//! made); approved ones go to Radarr or Sonarr ([`arr`]) where they are
//! connected, and a request is available once a library has it: the movie,
//! or every season asked for, whole. See docs/API.md, Requests.

pub mod arr;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::accounts::avatar_url;
use crate::auth::AuthUser;
use crate::catalog::{ME_CTE, VISIBLE};
use crate::db::Db;
use crate::discover::{self, CastMember, Details, Discover, Found, List, TitleKind, image_url};
use crate::error::AppError;
use crate::events::{EventBus, ServerEvent};
use crate::metadata::{MetadataAgent, TmdbError};
use crate::settings::{Settings, SettingsStore};
use crate::util::{now, utc};
use arr::{Arr, Download, Service};

/// What the limits count: requests of the last 7 days.
pub const LIMIT_WINDOW: i64 = 7 * 86_400;
/// How often the libraries are looked at for what is asked for (and after
/// every scan).
const SYNC_EVERY: Duration = Duration::from_secs(300);
/// Statuses of requests still asking for their title.
const ACTIVE: &str = "('pending', 'approved', 'failed')";
/// Requests listed at once, by default and at most.
const LIST_LIMIT: i64 = 50;
const LIST_MAX: i64 = 200;
/// A reason given for declining, and a root folder chosen, at most.
const REASON_MAX: usize = 500;
const FOLDER_MAX: usize = 512;
/// Titles whose other ids are looked up at once.
const LOOKUPS: usize = 8;

/// Whether an account may ask for titles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestAccess {
    /// May not ask.
    Off,
    /// Asks an administrator.
    Ask,
    /// Approved as asked.
    Auto,
}

impl RequestAccess {
    pub fn as_str(self) -> &'static str {
        match self {
            RequestAccess::Off => "off",
            RequestAccess::Ask => "ask",
            RequestAccess::Auto => "auto",
        }
    }

    /// As stored (anything else asks).
    pub fn parse(text: &str) -> RequestAccess {
        match text {
            "off" => RequestAccess::Off,
            "auto" => RequestAccess::Auto,
            _ => RequestAccess::Ask,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestStatus {
    Pending,
    Approved,
    Declined,
    Failed,
    Available,
}

impl RequestStatus {
    pub const ALL: [RequestStatus; 5] = [
        RequestStatus::Pending,
        RequestStatus::Approved,
        RequestStatus::Declined,
        RequestStatus::Failed,
        RequestStatus::Available,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            RequestStatus::Pending => "pending",
            RequestStatus::Approved => "approved",
            RequestStatus::Declined => "declined",
            RequestStatus::Failed => "failed",
            RequestStatus::Available => "available",
        }
    }

    pub fn parse(text: &str) -> Option<RequestStatus> {
        RequestStatus::ALL.into_iter().find(|s| s.as_str() == text)
    }
}

/// What a request endpoint answers when it can't: requests off (`503`,
/// `setup: false`), TMDB refusing or away (`503`, `setup: true`), or an
/// API error.
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("requests are off: an administrator can turn them on in Settings → Requests")]
    Off,
    #[error("{0}")]
    Tmdb(String),
    #[error(transparent)]
    App(#[from] AppError),
}

pub type RequestResult<T> = Result<T, RequestError>;

impl From<sqlx::Error> for RequestError {
    fn from(err: sqlx::Error) -> Self {
        RequestError::App(err.into())
    }
}

impl From<TmdbError> for RequestError {
    fn from(err: TmdbError) -> Self {
        match err {
            TmdbError::NotFound => RequestError::App(AppError::not_found("TMDB doesn't know this title")),
            TmdbError::Unauthorized => {
                RequestError::Tmdb("TMDB rejected the API key: an administrator can check it in Settings".into())
            }
            TmdbError::Other(err) => {
                tracing::warn!("TMDB: {err:#}");
                RequestError::Tmdb("TMDB couldn't be reached: try again in a moment".into())
            }
        }
    }
}

impl IntoResponse for RequestError {
    fn into_response(self) -> Response {
        let setup = match self {
            RequestError::App(err) => return err.into_response(),
            RequestError::Off => false,
            RequestError::Tmdb(_) => true,
        };
        let body = json!({ "code": "unavailable", "message": self.to_string(), "setup": setup });
        (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
    }
}

// ------------------------------------------------------------------ what the API shows

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub enabled: bool,
    pub access: RequestAccess,
    pub movies_left: Option<i64>,
    pub seasons_left: Option<i64>,
    /// With none left of a kind: when one more may be asked for.
    pub movies_next_at: Option<i64>,
    pub seasons_next_at: Option<i64>,
    pub pending: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TitleStatus {
    Library,
    Requested,
    None,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverTitle {
    pub kind: TitleKind,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub overview: String,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub rating: Option<f64>,
    pub status: TitleStatus,
    pub item_id: Option<i64>,
    pub request_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverPage {
    pub page: u32,
    pub total_pages: u32,
    pub results: Vec<DiscoverTitle>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverDetail {
    #[serde(flatten)]
    pub title: DiscoverTitle,
    pub original_title: Option<String>,
    pub tagline: Option<String>,
    pub genres: Vec<String>,
    pub release_date: Option<String>,
    pub runtime_minutes: Option<i64>,
    pub certification: Option<String>,
    pub network: Option<String>,
    pub show_status: Option<String>,
    pub anime: bool,
    pub cast: Vec<CastMember>,
    pub seasons: Option<Vec<DiscoverSeason>>,
    pub requests: Vec<MediaRequest>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverSeason {
    pub number: i64,
    pub name: String,
    pub episodes: i64,
    pub aired: i64,
    pub air_date: Option<String>,
    pub poster_url: Option<String>,
    pub in_library: i64,
    pub requested: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaRequest {
    pub id: i64,
    pub kind: TitleKind,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub seasons: Option<Vec<i64>>,
    pub available_seasons: Vec<i64>,
    pub anime: bool,
    pub status: RequestStatus,
    pub reason: Option<String>,
    pub requested_by: RequestUser,
    pub decided_by: Option<RequestUser>,
    pub decided_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub available_at: Option<i64>,
    pub item_id: Option<i64>,
    pub download: Option<Download>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestUser {
    pub id: i64,
    pub name: String,
    pub avatar_url: Option<String>,
    pub avatar_color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestList {
    pub total: i64,
    pub counts: BTreeMap<RequestStatus, i64>,
    pub requests: Vec<MediaRequest>,
}

/// `POST /api/requests`.
#[derive(Debug, Clone, Deserialize)]
pub struct NewRequest {
    pub kind: TitleKind,
    pub tmdb_id: i64,
    #[serde(default)]
    pub seasons: Option<Vec<i64>>,
}

/// What an approval may choose instead of the settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Approval {
    #[serde(default)]
    pub quality_profile: Option<i64>,
    #[serde(default)]
    pub root_folder: Option<String>,
}

/// Which requests `GET /api/requests` lists.
#[derive(Debug, Clone, Default)]
pub struct ListQuery {
    pub status: Option<RequestStatus>,
    /// Administrators: their own only.
    pub mine: bool,
    pub offset: i64,
    pub limit: Option<i64>,
}

// ------------------------------------------------------------------ rows

const COLUMNS: &str = "r.id, r.kind, r.tmdb_id, r.tvdb_id, r.imdb_id, r.title, r.year, r.poster_path,
    r.backdrop_path, r.seasons, r.available_seasons, r.anime, r.user_id, r.status, r.reason, r.decided_by,
    r.decided_at, r.item_id, r.arr_id, r.created_at, r.updated_at, r.available_at,
    COALESCE(NULLIF(TRIM(u.display_name), ''), u.username) AS maker_name, u.avatar AS maker_avatar,
    u.avatar_color AS maker_color, COALESCE(NULLIF(TRIM(d.display_name), ''), d.username) AS decider_name,
    d.avatar AS decider_avatar, d.avatar_color AS decider_color";
const TABLES: &str = "requests r JOIN users u ON u.id = r.user_id LEFT JOIN users d ON d.id = r.decided_by";

#[derive(Debug, Clone, sqlx::FromRow)]
struct RequestRow {
    id: i64,
    kind: String,
    tmdb_id: i64,
    tvdb_id: Option<i64>,
    imdb_id: Option<String>,
    title: String,
    year: Option<i64>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    seasons: Option<String>,
    available_seasons: String,
    anime: bool,
    user_id: i64,
    status: String,
    reason: Option<String>,
    decided_by: Option<i64>,
    decided_at: Option<i64>,
    item_id: Option<i64>,
    arr_id: Option<i64>,
    created_at: i64,
    updated_at: i64,
    available_at: Option<i64>,
    maker_name: String,
    maker_avatar: Option<String>,
    maker_color: Option<String>,
    decider_name: Option<String>,
    decider_avatar: Option<String>,
    decider_color: Option<String>,
}

impl RequestRow {
    fn kind(&self) -> TitleKind {
        TitleKind::parse(&self.kind).unwrap_or(TitleKind::Movie)
    }

    fn status(&self) -> RequestStatus {
        RequestStatus::parse(&self.status).unwrap_or(RequestStatus::Pending)
    }

    fn seasons(&self) -> Option<Vec<i64>> {
        self.seasons.as_deref().map(seasons_of)
    }

    fn known(&self) -> Known {
        Known { tmdb_id: self.tmdb_id, tvdb_id: self.tvdb_id, imdb_id: self.imdb_id.clone() }
    }

    /// As `viewer` sees it: the library's title where they may open it.
    fn to_dto(&self, visible: &HashSet<i64>, download: Option<Download>) -> MediaRequest {
        MediaRequest {
            id: self.id,
            kind: self.kind(),
            tmdb_id: self.tmdb_id,
            title: self.title.clone(),
            year: self.year,
            poster_url: image_url(self.poster_path.as_deref(), "w342"),
            backdrop_url: image_url(self.backdrop_path.as_deref(), "w1280"),
            seasons: self.seasons(),
            available_seasons: seasons_of(&self.available_seasons),
            anime: self.anime,
            status: self.status(),
            reason: self.reason.clone(),
            requested_by: RequestUser {
                id: self.user_id,
                name: self.maker_name.clone(),
                avatar_url: avatar_url(self.user_id, self.maker_avatar.as_deref()),
                avatar_color: self.maker_color.clone(),
            },
            decided_by: self.decided_by.zip(self.decider_name.clone()).map(|(id, name)| RequestUser {
                id,
                name,
                avatar_url: avatar_url(id, self.decider_avatar.as_deref()),
                avatar_color: self.decider_color.clone(),
            }),
            decided_at: self.decided_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
            available_at: self.available_at,
            item_id: self.item_id.filter(|id| visible.contains(id)),
            download,
        }
    }
}

fn seasons_of(json: &str) -> Vec<i64> {
    serde_json::from_str(json).unwrap_or_default()
}

fn seasons_json(seasons: &[i64]) -> String {
    serde_json::to_string(seasons).unwrap_or_else(|_| "[]".to_owned())
}

/// What a title is known by: TMDB's id, and for shows TheTVDB's and IMDb's.
#[derive(Debug, Clone)]
struct Known {
    tmdb_id: i64,
    tvdb_id: Option<i64>,
    imdb_id: Option<String>,
}

impl Known {
    fn of(details: &Details) -> Known {
        Known { tmdb_id: details.tmdb_id, tvdb_id: details.tvdb_id, imdb_id: details.imdb_id.clone() }
    }
}

/// What the libraries and requests have of a title, for one account.
#[derive(Debug, Clone, Copy, Default)]
struct Mark {
    library: bool,
    item_id: Option<i64>,
    requested: bool,
    request_id: Option<i64>,
}

impl Mark {
    fn status(&self) -> TitleStatus {
        if self.library {
            TitleStatus::Library
        } else if self.requested {
            TitleStatus::Requested
        } else {
            TitleStatus::None
        }
    }
}

fn title_of(found: &Found, mark: Mark) -> DiscoverTitle {
    DiscoverTitle {
        kind: found.kind,
        tmdb_id: found.tmdb_id,
        title: found.title.clone(),
        year: found.year,
        overview: found.overview.clone(),
        poster_url: image_url(found.poster_path.as_deref(), "w342"),
        backdrop_url: image_url(found.backdrop_path.as_deref(), "w1280"),
        rating: found.rating,
        status: mark.status(),
        item_id: mark.item_id,
        request_id: mark.request_id,
    }
}

/// Whether a library has a season whole: as many episodes as have aired.
/// `Maybe`: fewer, but the last one aired, as with files of several
/// episodes each (or some missing): Sonarr can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Whole {
    Yes,
    Maybe,
    No,
}

/// `counts`: episodes a library has of each season, with the highest
/// episode number.
fn whole(counts: &HashMap<i64, (i64, i64)>, season: &discover::Season) -> Whole {
    let (episodes, last) = counts.get(&season.number).copied().unwrap_or_default();
    if season.aired <= 0 {
        Whole::No
    } else if episodes >= season.aired {
        Whole::Yes
    } else if episodes > 0 && last >= season.aired {
        Whole::Maybe
    } else {
        Whole::No
    }
}

/// How much more an account may ask for in [`LIMIT_WINDOW`].
#[derive(Debug, Clone, Default)]
struct Quota {
    movies_left: Option<i64>,
    seasons_left: Option<i64>,
    /// When the oldest request counted of each kind stops counting.
    movies_free_at: Option<i64>,
    seasons_free_at: Option<i64>,
}

impl Quota {
    /// With none left of `kind`: when the oldest request counted stops
    /// counting, and one more may be asked for.
    fn next_at(&self, kind: TitleKind) -> Option<i64> {
        let (left, free_at) = match kind {
            TitleKind::Movie => (self.movies_left, self.movies_free_at),
            TitleKind::Show => (self.seasons_left, self.seasons_free_at),
        };
        free_at.filter(|_| left == Some(0))
    }

    /// Refuses asking for `count` of `kind` past the limit.
    fn allows(&self, kind: TitleKind, count: i64) -> Result<(), AppError> {
        let (left, free_at, one, many) = match kind {
            TitleKind::Movie => (self.movies_left, self.movies_free_at, "movie", "movies"),
            TitleKind::Show => (self.seasons_left, self.seasons_free_at, "season", "seasons"),
        };
        let Some(left) = left.filter(|left| count > *left) else { return Ok(()) };
        let from = free_at.map(|at| format!(" from {}", day(at))).unwrap_or_default();
        Err(AppError::TooManyRequests(match left {
            0 => format!("this account has asked for all the {many} it may in 7 days: it may ask again{from}"),
            1 => format!("this account may ask for 1 more {one} for now"),
            n => format!("this account may ask for {n} more {many} for now"),
        }))
    }
}

/// `2026-11-03 14:05 UTC`.
fn day(secs: i64) -> String {
    let [y, m, d, h, min, _] = utc(secs);
    format!("{y}-{m:02}-{d:02} {h:02}:{min:02} UTC")
}

// ------------------------------------------------------------------ the service

pub struct Requests {
    db: Db,
    settings: Arc<SettingsStore>,
    events: EventBus,
    discover: Discover,
    arr: Arr,
    /// Requests are made one at a time: two at once can't both find a title
    /// not asked for yet.
    making: tokio::sync::Mutex<()>,
    /// One look at the libraries at a time.
    syncing: tokio::sync::Mutex<()>,
}

impl Requests {
    pub fn new(db: Db, settings: Arc<SettingsStore>, events: EventBus, metadata: Arc<MetadataAgent>) -> Arc<Self> {
        Arc::new(Self {
            discover: Discover::new(metadata, settings.clone()),
            arr: Arr::new(),
            db,
            settings,
            events,
            making: Default::default(),
            syncing: Default::default(),
        })
    }

    pub fn arr(&self) -> &Arr {
        &self.arr
    }

    /// Requests are on, and TMDB has a key.
    pub fn enabled(&self) -> bool {
        let settings = self.settings.get();
        settings.requests && !settings.tmdb_api_key.is_empty()
    }

    fn on(&self) -> RequestResult<()> {
        if self.enabled() { Ok(()) } else { Err(RequestError::Off) }
    }

    /// Whether `user` may ask: administrators are approved as they ask.
    pub async fn access(&self, user: &AuthUser) -> sqlx::Result<RequestAccess> {
        if user.is_admin {
            return Ok(RequestAccess::Auto);
        }
        let access: Option<String> = sqlx::query_scalar("SELECT request_access FROM users WHERE id = ?")
            .bind(user.id)
            .fetch_optional(&self.db.read)
            .await?;
        Ok(access.as_deref().map_or(RequestAccess::Off, RequestAccess::parse))
    }

    /// Requests are on and `user` may ask: how.
    async fn asker(&self, user: &AuthUser) -> RequestResult<RequestAccess> {
        self.on()?;
        match self.access(user).await? {
            RequestAccess::Off => Err(AppError::forbidden(
                "this account may not ask for titles: an administrator can allow it in Settings → Users",
            )
            .into()),
            access => Ok(access),
        }
    }

    fn changed(&self, user_id: i64) {
        self.events.send(ServerEvent::RequestsChanged { user_id });
    }

    // -------------------------------------------------------------- reading

    /// `GET /api/requests/summary`.
    pub async fn summary(&self, user: &AuthUser) -> sqlx::Result<Summary> {
        let access = self.access(user).await?;
        let quota = if user.is_admin { Quota::default() } else { self.quota(user.id, &self.settings.get()).await? };
        let pending = if user.is_admin {
            sqlx::query_scalar("SELECT COUNT(*) FROM requests WHERE status = 'pending'")
                .fetch_one(&self.db.read)
                .await?
        } else {
            0
        };
        Ok(Summary {
            enabled: self.enabled(),
            access,
            movies_left: quota.movies_left,
            seasons_left: quota.seasons_left,
            movies_next_at: quota.next_at(TitleKind::Movie),
            seasons_next_at: quota.next_at(TitleKind::Show),
            pending,
        })
    }

    async fn quota(&self, user_id: i64, settings: &Settings) -> sqlx::Result<Quota> {
        let rows: Vec<(String, Option<String>, i64)> = sqlx::query_as(
            "SELECT kind, seasons, created_at FROM requests
             WHERE user_id = ? AND created_at > ? AND status != 'declined' ORDER BY created_at",
        )
        .bind(user_id)
        .bind(now() - LIMIT_WINDOW)
        .fetch_all(&self.db.read)
        .await?;
        let (mut movies, mut seasons) = ((0, None), (0, None));
        for (kind, list, at) in rows {
            let (used, oldest): &mut (i64, Option<i64>) = if kind == "movie" { &mut movies } else { &mut seasons };
            *used += if kind == "movie" { 1 } else { list.as_deref().map_or(0, |l| seasons_of(l).len() as i64) };
            if oldest.is_none() {
                *oldest = Some(at + LIMIT_WINDOW);
            }
        }
        let left = |limit: u32, used: i64| (limit > 0).then(|| (i64::from(limit) - used).max(0));
        Ok(Quota {
            movies_left: left(settings.request_limit_movies, movies.0),
            seasons_left: left(settings.request_limit_seasons, seasons.0),
            movies_free_at: movies.1,
            seasons_free_at: seasons.1,
        })
    }

    /// `GET /api/discover/{list}`.
    pub async fn discover(&self, user: &AuthUser, list: List, page: u32) -> RequestResult<DiscoverPage> {
        self.asker(user).await?;
        let found = self.discover.list(list, page).await?;
        self.page(user, &found).await
    }

    /// `GET /api/discover/search`.
    pub async fn search(&self, user: &AuthUser, text: &str, page: u32) -> RequestResult<DiscoverPage> {
        self.asker(user).await?;
        let found = self.discover.search(text, page).await?;
        self.page(user, &found).await
    }

    async fn page(&self, user: &AuthUser, found: &discover::Page) -> RequestResult<DiscoverPage> {
        let marks = self.marks(user, &found.results).await?;
        let results = found
            .results
            .iter()
            .map(|t| title_of(t, marks.get(&(t.kind, t.tmdb_id)).copied().unwrap_or_default()))
            .collect();
        Ok(DiscoverPage { page: found.page, total_pages: found.total_pages, results })
    }

    /// What the libraries and requests have of `titles`, for `user`.
    async fn marks(&self, user: &AuthUser, titles: &[Found]) -> sqlx::Result<HashMap<(TitleKind, i64), Mark>> {
        let mut marks: HashMap<(TitleKind, i64), Mark> = HashMap::new();
        for kind in [TitleKind::Movie, TitleKind::Show] {
            let known: Vec<Known> = titles
                .iter()
                .filter(|t| t.kind == kind)
                .map(|t| Known { tmdb_id: t.tmdb_id, tvdb_id: None, imdb_id: None })
                .collect();
            if known.is_empty() {
                continue;
            }
            let mut found = self.in_library(user.id, kind, &known).await?;
            // Shows a TV source other than TMDB named have no TMDB id: the
            // ones not found are looked for by their other ids.
            if kind == TitleKind::Show && self.shows_without_tmdb().await? {
                let missing: Vec<usize> = (0..known.len()).filter(|i| !found.iter().any(|f| f.0 == *i)).collect();
                // Made before they run: a stream mapping borrowed indexes
                // would keep the handlers' futures from being `Send`.
                let lookups: Vec<_> = missing.iter().map(|i| self.discover.show_ids(known[*i].tmdb_id)).collect();
                let ids: Vec<_> = futures::stream::iter(lookups).buffered(LOOKUPS).collect().await;
                let (mut others, mut index) = (Vec::new(), Vec::new());
                for (i, ids) in missing.iter().zip(ids) {
                    match ids {
                        Ok(ids) if ids.tvdb_id.is_some() || ids.imdb_id.is_some() => {
                            others.push(Known {
                                tmdb_id: known[*i].tmdb_id,
                                tvdb_id: ids.tvdb_id,
                                imdb_id: ids.imdb_id.clone(),
                            });
                            index.push(*i);
                        }
                        Ok(_) => {}
                        Err(err) => tracing::debug!("TMDB: a show's ids: {err}"),
                    }
                }
                if !others.is_empty() {
                    let more = self.in_library(user.id, kind, &others).await?;
                    found.extend(more.into_iter().map(|(j, item, visible)| (index[j], item, visible)));
                }
            }
            for (i, item, visible) in found {
                let mark = marks.entry((kind, known[i].tmdb_id)).or_default();
                mark.library = true;
                if visible && mark.item_id.is_none() {
                    mark.item_id = Some(item);
                }
            }
            let ids: Vec<i64> = known.iter().map(|k| k.tmdb_id).collect();
            for (tmdb_id, id, maker) in self.active_requests(kind, &ids).await? {
                let mark = marks.entry((kind, tmdb_id)).or_default();
                mark.requested = true;
                if mark.request_id.is_none() && (user.is_admin || maker == user.id) {
                    mark.request_id = Some(id);
                }
            }
        }
        Ok(marks)
    }

    /// Library titles of `kind` that are any of `titles` (by TMDB's id, or
    /// TheTVDB's or IMDb's where known): the title's index in `titles`, the
    /// library's title, and whether `user_id` may open it.
    async fn in_library(
        &self,
        user_id: i64,
        kind: TitleKind,
        titles: &[Known],
    ) -> sqlx::Result<Vec<(usize, i64, bool)>> {
        let tvdb: Vec<i64> = titles.iter().filter_map(|t| t.tvdb_id).collect();
        let imdb: Vec<&str> = titles.iter().filter_map(|t| t.imdb_id.as_deref()).collect();
        let list = |n: usize| vec!["?"; n].join(",");
        let sql = format!(
            "{ME_CTE} SELECT i.id, i.tmdb_id, i.tvdb_id, i.imdb_id, CASE WHEN {VISIBLE} THEN 1 ELSE 0 END
             FROM items i LEFT JOIN items p ON p.id = i.parent_id LEFT JOIN items gp ON gp.id = i.grandparent_id
             WHERE i.kind = ? AND (i.tmdb_id IN ({}) OR i.tvdb_id IN ({}) OR i.imdb_id IN ({}))
             ORDER BY i.id",
            list(titles.len()),
            list(tvdb.len()),
            list(imdb.len()),
        );
        let mut query = sqlx::query_as::<_, (i64, Option<i64>, Option<i64>, Option<String>, bool)>(
            sqlx::AssertSqlSafe(sql.as_str()),
        )
        .bind(user_id)
        .bind(kind.as_str());
        for title in titles {
            query = query.bind(title.tmdb_id);
        }
        for id in &tvdb {
            query = query.bind(id);
        }
        for id in &imdb {
            query = query.bind(*id);
        }
        let mut found = Vec::new();
        for (item, tmdb_id, tvdb_id, imdb_id, visible) in query.fetch_all(&self.db.read).await? {
            for (i, title) in titles.iter().enumerate() {
                let same = tmdb_id == Some(title.tmdb_id)
                    || (title.tvdb_id.is_some() && tvdb_id == title.tvdb_id)
                    || (title.imdb_id.is_some() && imdb_id == title.imdb_id);
                if same {
                    found.push((i, item, visible));
                }
            }
        }
        Ok(found)
    }

    /// Whether some library show has no TMDB id (another TV source named
    /// it): shows are then looked for by their other ids too.
    async fn shows_without_tmdb(&self) -> sqlx::Result<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM items WHERE kind = 'show' AND tmdb_id IS NULL
                            AND (tvdb_id IS NOT NULL OR imdb_id IS NOT NULL))",
        )
        .fetch_one(&self.db.read)
        .await
    }

    /// Active requests for any of these titles, newest first: TMDB's id,
    /// the request's, and its maker's.
    async fn active_requests(&self, kind: TitleKind, tmdb_ids: &[i64]) -> sqlx::Result<Vec<(i64, i64, i64)>> {
        let sql = format!(
            "SELECT tmdb_id, id, user_id FROM requests
             WHERE kind = ? AND tmdb_id IN ({}) AND status IN {ACTIVE} ORDER BY id DESC",
            vec!["?"; tmdb_ids.len()].join(",")
        );
        let mut query = sqlx::query_as::<_, (i64, i64, i64)>(sqlx::AssertSqlSafe(sql.as_str())).bind(kind.as_str());
        for id in tmdb_ids {
            query = query.bind(id);
        }
        query.fetch_all(&self.db.read).await
    }

    /// Episodes the libraries have of each season of these shows (the most
    /// any one of them has), with the highest episode number.
    async fn season_counts(&self, shows: &[i64]) -> sqlx::Result<HashMap<i64, (i64, i64)>> {
        if shows.is_empty() {
            return Ok(HashMap::new());
        }
        let sql = format!(
            "SELECT s.idx, COUNT(e.id), COALESCE(MAX(e.idx), 0) FROM items s
               JOIN items e ON e.parent_id = s.id AND e.kind = 'episode'
             WHERE s.parent_id IN ({}) AND s.kind = 'season' AND s.idx IS NOT NULL
             GROUP BY s.parent_id, s.idx",
            vec!["?"; shows.len()].join(",")
        );
        let mut query = sqlx::query_as::<_, (i64, i64, i64)>(sqlx::AssertSqlSafe(sql.as_str()));
        for id in shows {
            query = query.bind(id);
        }
        let mut counts: HashMap<i64, (i64, i64)> = HashMap::new();
        for (season, episodes, last) in query.fetch_all(&self.db.read).await? {
            let entry = counts.entry(season).or_default();
            *entry = (entry.0.max(episodes), entry.1.max(last));
        }
        Ok(counts)
    }

    /// `GET /api/discover/movie/{tmdb_id}` and `…/show/{tmdb_id}`.
    pub async fn detail(&self, user: &AuthUser, kind: TitleKind, tmdb_id: i64) -> RequestResult<DiscoverDetail> {
        self.asker(user).await?;
        let details = self.discover.details(kind, tmdb_id).await?;
        let library = self.in_library(user.id, kind, &[Known::of(&details)]).await?;
        let active = self.active_requests(kind, &[tmdb_id]).await?;
        let mark = Mark {
            library: !library.is_empty(),
            item_id: library.iter().find(|l| l.2).map(|l| l.1),
            requested: !active.is_empty(),
            request_id: active.iter().find(|a| user.is_admin || a.2 == user.id).map(|a| a.1),
        };
        let seasons = match kind {
            TitleKind::Movie => None,
            TitleKind::Show => {
                let shows: Vec<i64> = library.iter().map(|l| l.1).collect();
                let counts = self.season_counts(&shows).await?;
                let asked = self.asked_seasons(tmdb_id).await?;
                Some(
                    details
                        .seasons
                        .iter()
                        .map(|s| DiscoverSeason {
                            number: s.number,
                            name: s.name.clone(),
                            episodes: s.episodes,
                            aired: s.aired,
                            air_date: s.air_date.clone(),
                            poster_url: image_url(s.poster_path.as_deref(), "w342"),
                            in_library: counts.get(&s.number).map_or(0, |c| c.0),
                            requested: asked.contains(&s.number),
                        })
                        .collect(),
                )
            }
        };
        let rows = self.title_rows(user, kind, tmdb_id).await?;
        let requests = self.dtos(user, rows).await?;
        let found = Found {
            kind,
            tmdb_id,
            title: details.title.clone(),
            year: details.year,
            overview: details.overview.clone(),
            poster_path: None,
            backdrop_path: details.backdrop_path.clone(),
            rating: details.rating,
        };
        let mut title = title_of(&found, mark);
        title.poster_url = image_url(details.poster_path.as_deref(), "w500");
        Ok(DiscoverDetail {
            title,
            original_title: details.original_title.clone(),
            tagline: details.tagline.clone(),
            genres: details.genres.clone(),
            release_date: details.release_date.clone(),
            runtime_minutes: details.runtime_minutes,
            certification: details.certification.clone(),
            network: details.network.clone(),
            show_status: details.show_status.clone(),
            anime: details.anime,
            cast: details.cast.clone(),
            seasons,
            requests,
        })
    }

    /// The seasons active requests ask for of a show.
    async fn asked_seasons(&self, tmdb_id: i64) -> sqlx::Result<HashSet<i64>> {
        let lists: Vec<Option<String>> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT seasons FROM requests WHERE kind = 'show' AND tmdb_id = ? AND status IN {ACTIVE}"
        )))
        .bind(tmdb_id)
        .fetch_all(&self.db.read)
        .await?;
        Ok(lists.iter().flatten().flat_map(|l| seasons_of(l)).collect())
    }

    /// Requests for a title: `user`'s, everyone's for administrators;
    /// newest first.
    async fn title_rows(&self, user: &AuthUser, kind: TitleKind, tmdb_id: i64) -> sqlx::Result<Vec<RequestRow>> {
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM {TABLES} WHERE r.kind = ? AND r.tmdb_id = ? AND (? OR r.user_id = ?)
             ORDER BY r.created_at DESC, r.id DESC"
        )))
        .bind(kind.as_str())
        .bind(tmdb_id)
        .bind(user.is_admin)
        .bind(user.id)
        .fetch_all(&self.db.read)
        .await
    }

    async fn row(&self, id: i64) -> sqlx::Result<Option<RequestRow>> {
        sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM {TABLES} WHERE r.id = ?")))
            .bind(id)
            .fetch_optional(&self.db.read)
            .await
    }

    /// Requests as `viewer` sees them, with their downloads.
    async fn dtos(&self, viewer: &AuthUser, rows: Vec<RequestRow>) -> sqlx::Result<Vec<MediaRequest>> {
        let items: Vec<i64> = rows.iter().filter_map(|r| r.item_id).collect();
        let visible = visible_items(&self.db, viewer.id, &items).await?;
        let downloads = self.downloads(&rows).await;
        Ok(rows.iter().map(|r| r.to_dto(&visible, downloads.get(&r.id).cloned())).collect())
    }

    /// The downloads of approved requests that Radarr or Sonarr is busy with.
    async fn downloads(&self, rows: &[RequestRow]) -> HashMap<i64, Download> {
        let settings = self.settings.get();
        let mut downloads = HashMap::new();
        for (service, kind) in [(Service::Radarr, TitleKind::Movie), (Service::Sonarr, TitleKind::Show)] {
            let wanted: Vec<(&RequestRow, i64)> = rows
                .iter()
                .filter(|r| r.kind() == kind && r.status() == RequestStatus::Approved)
                .filter_map(|r| Some((r, r.arr_id?)))
                .collect();
            let Some(to) = service.saved(&settings).filter(|_| !wanted.is_empty()) else { continue };
            let Ok(queue) = self.arr.queue(service, &to).await else { continue };
            for (row, arr_id) in wanted {
                let seasons = row.seasons();
                if let Some(download) = arr::download(&arr::records_of(&queue, arr_id, seasons.as_deref())) {
                    downloads.insert(row.id, download);
                }
            }
        }
        downloads
    }

    /// `GET /api/requests`.
    pub async fn list(&self, user: &AuthUser, query: ListQuery) -> RequestResult<RequestList> {
        self.asker(user).await?;
        let everyone = user.is_admin && !query.mine;
        let counted: Vec<(String, i64)> =
            sqlx::query_as("SELECT r.status, COUNT(*) FROM requests r WHERE (? OR r.user_id = ?) GROUP BY r.status")
                .bind(everyone)
                .bind(user.id)
                .fetch_all(&self.db.read)
                .await?;
        let mut counts: BTreeMap<RequestStatus, i64> = RequestStatus::ALL.into_iter().map(|s| (s, 0)).collect();
        for (status, count) in counted {
            if let Some(status) = RequestStatus::parse(&status) {
                counts.insert(status, count);
            }
        }
        let total = match query.status {
            Some(status) => counts[&status],
            None => counts.values().sum(),
        };
        let limit = query.limit.unwrap_or(LIST_LIMIT).clamp(1, LIST_MAX);
        let rows: Vec<RequestRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM {TABLES} WHERE (? OR r.user_id = ?) AND (? IS NULL OR r.status = ?)
             ORDER BY r.created_at DESC, r.id DESC LIMIT ? OFFSET ?"
        )))
        .bind(everyone)
        .bind(user.id)
        .bind(query.status.map(RequestStatus::as_str))
        .bind(query.status.map(RequestStatus::as_str))
        .bind(limit)
        .bind(query.offset.max(0))
        .fetch_all(&self.db.read)
        .await?;
        Ok(RequestList { total, counts, requests: self.dtos(user, rows).await? })
    }

    /// `GET /api/requests/{id}`.
    pub async fn get(&self, user: &AuthUser, id: i64) -> RequestResult<MediaRequest> {
        self.asker(user).await?;
        self.one(user, id).await
    }

    /// A request `user` may see.
    async fn one(&self, user: &AuthUser, id: i64) -> RequestResult<MediaRequest> {
        let row = self.row(id).await?.filter(|r| user.is_admin || r.user_id == user.id);
        let row = row.ok_or_else(|| AppError::not_found("no such request"))?;
        Ok(self.dtos(user, vec![row]).await?.remove(0))
    }

    // -------------------------------------------------------------- changing

    /// `POST /api/requests`.
    pub async fn create(&self, user: &AuthUser, new: NewRequest) -> RequestResult<MediaRequest> {
        let access = self.asker(user).await?;
        if new.kind == TitleKind::Movie && new.seasons.is_some() {
            return Err(AppError::bad_request("movies have no seasons").into());
        }
        if new.tmdb_id <= 0 {
            return Err(AppError::not_found("TMDB doesn't know this title").into());
        }
        let details = self.discover.details(new.kind, new.tmdb_id).await?;
        let making = self.making.lock().await;
        let library = self.in_library(user.id, new.kind, &[Known::of(&details)]).await?;
        let active = self.active_requests(new.kind, &[new.tmdb_id]).await?;
        let seasons = match new.kind {
            TitleKind::Movie if !library.is_empty() => {
                return Err(AppError::conflict("a library has this movie already").into());
            }
            TitleKind::Movie if !active.is_empty() => {
                return Err(AppError::conflict("this movie is asked for already").into());
            }
            TitleKind::Movie => None,
            TitleKind::Show => {
                let listed: Vec<i64> = details.seasons.iter().map(|s| s.number).collect();
                let mut wanted = match new.seasons {
                    Some(asked) => {
                        if let Some(n) = asked.iter().find(|n| !listed.contains(n)) {
                            let message = format!("TMDB lists no season {n} of {}", details.title);
                            return Err(AppError::bad_request(message).into());
                        }
                        asked
                    }
                    None => listed.iter().copied().filter(|n| *n > 0).collect(),
                };
                wanted.sort_unstable();
                wanted.dedup();
                if wanted.is_empty() {
                    return Err(AppError::bad_request("ask for at least one season").into());
                }
                let shows: Vec<i64> = library.iter().map(|l| l.1).collect();
                let counts = self.season_counts(&shows).await?;
                let asked = self.asked_seasons(new.tmdb_id).await?;
                let there = |n: i64| details.seasons.iter().any(|s| s.number == n && whole(&counts, s) == Whole::Yes);
                let left: Vec<i64> = wanted.into_iter().filter(|n| !asked.contains(n) && !there(*n)).collect();
                if left.is_empty() {
                    let message = "a library has these seasons whole, or they're asked for already";
                    return Err(AppError::conflict(message).into());
                }
                Some(left)
            }
        };
        if !user.is_admin {
            let quota = self.quota(user.id, &self.settings.get()).await?;
            quota.allows(new.kind, seasons.as_ref().map_or(1, |s| s.len() as i64))?;
        }
        let at = now();
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO requests (kind, tmdb_id, tvdb_id, imdb_id, title, year, poster_path, backdrop_path, seasons,
                                   anime, user_id, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?) RETURNING id",
        )
        .bind(new.kind.as_str())
        .bind(new.tmdb_id)
        .bind(details.tvdb_id)
        .bind(details.imdb_id.as_deref())
        .bind(&details.title)
        .bind(details.year)
        .bind(details.poster_path.as_deref())
        .bind(details.backdrop_path.as_deref())
        .bind(seasons.as_deref().map(seasons_json))
        .bind(details.anime)
        .bind(user.id)
        .bind(at)
        .bind(at)
        .fetch_one(&self.db.write)
        .await?;
        drop(making);
        tracing::info!("{} asked for {} (request {id})", user.username, details.title);
        self.changed(user.id);
        if access == RequestAccess::Auto {
            self.send(id, user.id, &Approval::default()).await?;
        }
        self.one(user, id).await
    }

    /// `POST /api/requests/{id}/approve`.
    pub async fn approve(&self, admin: &AuthUser, id: i64, approval: Approval) -> RequestResult<MediaRequest> {
        self.on()?;
        let approval = Approval {
            quality_profile: approval.quality_profile,
            root_folder: approval.root_folder.map(|f| f.trim().to_owned()),
        };
        if approval.quality_profile.is_some_and(|p| p <= 0) {
            return Err(AppError::bad_request("a quality profile is chosen by its id").into());
        }
        if approval.root_folder.as_deref().is_some_and(|f| f.is_empty() || f.chars().count() > FOLDER_MAX) {
            return Err(AppError::bad_request("a root folder is a folder's path").into());
        }
        let row = self.row(id).await?.ok_or_else(|| AppError::not_found("no such request"))?;
        if !self.send(id, admin.id, &approval).await? {
            let message = format!("this request is {}: only pending and failed ones are approved", row.status);
            return Err(AppError::conflict(message).into());
        }
        self.one(admin, id).await
    }

    /// Approves a pending or failed request as `decider`, and adds it to
    /// Radarr or Sonarr where connected (should that fail, the request is
    /// `failed`, saying why). `false`: it was neither pending nor failed.
    async fn send(&self, id: i64, decider: i64, approval: &Approval) -> sqlx::Result<bool> {
        let at = now();
        let approved = sqlx::query(
            "UPDATE requests SET status = 'approved', reason = NULL, decided_by = ?, decided_at = ?, updated_at = ?
             WHERE id = ? AND status IN ('pending', 'failed')",
        )
        .bind(decider)
        .bind(at)
        .bind(at)
        .bind(id)
        .execute(&self.db.write)
        .await?
        .rows_affected()
            == 1;
        if !approved {
            return Ok(false);
        }
        let Some(row) = self.row(id).await? else { return Ok(true) };
        let settings = self.settings.get();
        let (service, kind) = match row.kind() {
            TitleKind::Movie => (Service::Radarr, TitleKind::Movie),
            TitleKind::Show => (Service::Sonarr, TitleKind::Show),
        };
        if let Some(to) = service.saved(&settings) {
            let seasons = row.seasons().unwrap_or_default();
            let title = arr::Title {
                tmdb_id: row.tmdb_id,
                tvdb_id: row.tvdb_id,
                anime: row.anime,
                seasons: &seasons,
                quality_profile: approval.quality_profile,
                root_folder: approval.root_folder.as_deref(),
            };
            let added = match kind {
                TitleKind::Movie => self.arr.add_movie(&to, &settings, &title).await,
                TitleKind::Show => self.arr.add_series(&to, &settings, &title).await,
            };
            match added {
                Ok(arr_id) => {
                    tracing::info!("request {id}: {} added to {}", row.title, service.name());
                    sqlx::query("UPDATE requests SET arr_id = ? WHERE id = ?")
                        .bind(arr_id)
                        .bind(id)
                        .execute(&self.db.write)
                        .await?;
                }
                Err(err) => {
                    tracing::warn!("request {id}: {} wasn't added to {}: {err}", row.title, service.name());
                    sqlx::query(
                        "UPDATE requests SET status = 'failed', reason = ?, updated_at = ?
                         WHERE id = ? AND status = 'approved'",
                    )
                    .bind(err.to_string())
                    .bind(now())
                    .bind(id)
                    .execute(&self.db.write)
                    .await?;
                }
            }
        }
        self.changed(row.user_id);
        Ok(true)
    }

    /// `POST /api/requests/{id}/decline`.
    pub async fn decline(&self, admin: &AuthUser, id: i64, reason: Option<String>) -> RequestResult<MediaRequest> {
        self.on()?;
        let reason = reason.map(|r| r.trim().to_owned()).filter(|r| !r.is_empty());
        if reason.as_ref().is_some_and(|r| r.chars().count() > REASON_MAX) {
            return Err(AppError::bad_request(format!("a reason is {REASON_MAX} characters at most")).into());
        }
        let row = self.row(id).await?.ok_or_else(|| AppError::not_found("no such request"))?;
        let at = now();
        let declined = sqlx::query(
            "UPDATE requests SET status = 'declined', reason = ?, decided_by = ?, decided_at = ?, updated_at = ?
             WHERE id = ? AND status IN ('pending', 'failed')",
        )
        .bind(reason)
        .bind(admin.id)
        .bind(at)
        .bind(at)
        .bind(id)
        .execute(&self.db.write)
        .await?
        .rows_affected();
        if declined == 0 {
            let message = format!("this request is {}: only pending and failed ones are declined", row.status);
            return Err(AppError::conflict(message).into());
        }
        self.changed(row.user_id);
        self.one(admin, id).await
    }

    /// `DELETE /api/requests/{id}`: its maker withdraws it while it isn't
    /// approved; administrators delete any.
    pub async fn delete(&self, user: &AuthUser, id: i64) -> RequestResult<()> {
        self.asker(user).await?;
        let row = self.row(id).await?.filter(|r| user.is_admin || r.user_id == user.id);
        let row = row.ok_or_else(|| AppError::not_found("no such request"))?;
        let deleted =
            sqlx::query("DELETE FROM requests WHERE id = ? AND (? OR status IN ('pending', 'declined', 'failed'))")
                .bind(id)
                .bind(user.is_admin)
                .execute(&self.db.write)
                .await?
                .rows_affected();
        if deleted == 0 {
            return Err(AppError::conflict("an approved request stays: an administrator can delete it").into());
        }
        self.changed(row.user_id);
        Ok(())
    }

    // -------------------------------------------------------------- availability

    /// Looks for what active requests ask for in the libraries: a request
    /// becomes available with its movie, or every season asked for whole.
    pub async fn sync(&self) -> anyhow::Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        let _one = self.syncing.lock().await;
        let rows: Vec<RequestRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM {TABLES} WHERE r.status IN {ACTIVE} ORDER BY r.id"
        )))
        .fetch_all(&self.db.read)
        .await?;
        let sonarr = Service::Sonarr.saved(&self.settings.get());
        for row in rows {
            // As its maker sees the library: theirs to open, where they may.
            let library = self.in_library(row.user_id, row.kind(), &[row.known()]).await?;
            let Some(item) = library.iter().find(|l| l.2).or(library.first()).map(|l| (l.1, l.2)) else { continue };
            let asked = row.seasons().unwrap_or_default();
            if row.kind() == TitleKind::Movie {
                self.available(&row, item, &asked).await?;
                continue;
            }
            let details = match self.discover.details(TitleKind::Show, row.tmdb_id).await {
                Ok(details) => details,
                Err(err) => {
                    tracing::debug!("request {}: TMDB: {err}", row.id);
                    continue;
                }
            };
            let shows: Vec<i64> = library.iter().map(|l| l.1).collect();
            let counts = self.season_counts(&shows).await?;
            let mut have = Vec::new();
            for n in &asked {
                let Some(season) = details.seasons.iter().find(|s| s.number == *n) else { continue };
                let whole = match whole(&counts, season) {
                    Whole::Yes => true,
                    Whole::Maybe => match (&sonarr, row.arr_id) {
                        (Some(to), Some(series)) => self.arr.season_complete(to, series, *n).await.unwrap_or(false),
                        _ => false,
                    },
                    Whole::No => false,
                };
                if whole {
                    have.push(*n);
                }
            }
            if !asked.is_empty() && have.len() == asked.len() {
                self.available(&row, item, &have).await?;
            } else if have != seasons_of(&row.available_seasons) || row.item_id != Some(item.0) {
                sqlx::query("UPDATE requests SET available_seasons = ?, item_id = ?, updated_at = ? WHERE id = ?")
                    .bind(seasons_json(&have))
                    .bind(item.0)
                    .bind(now())
                    .bind(row.id)
                    .execute(&self.db.write)
                    .await?;
                self.changed(row.user_id);
            }
        }
        Ok(())
    }

    /// A request's title is in a library: `item`, and whether its maker may
    /// open it.
    async fn available(&self, row: &RequestRow, item: (i64, bool), seasons: &[i64]) -> sqlx::Result<()> {
        let at = now();
        let done = sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE requests SET status = 'available', item_id = ?, available_seasons = ?, available_at = ?,
                 updated_at = ?
             WHERE id = ? AND status IN {ACTIVE}"
        )))
        .bind(item.0)
        .bind(seasons_json(seasons))
        .bind(at)
        .bind(at)
        .bind(row.id)
        .execute(&self.db.write)
        .await?
        .rows_affected();
        if done == 1 {
            tracing::info!("request {}: {} is available", row.id, row.title);
            self.events.send(ServerEvent::RequestAvailable {
                user_id: row.user_id,
                request_id: row.id,
                title: row.title.clone(),
                item_id: item.1.then_some(item.0),
            });
            self.changed(row.user_id);
        }
        Ok(())
    }

    /// Looks at the libraries every 5 minutes, and after each scan, until
    /// the server stops.
    pub fn start(self: &Arc<Self>, events: &EventBus, stopping: CancellationToken) {
        let (requests, mut events) = (self.clone(), events.subscribe());
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(SYNC_EVERY);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = tick.tick() => {}
                    event = events.recv() => match event {
                        Ok(ServerEvent::ScanFinished { .. }) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        _ => continue,
                    },
                    () = stopping.cancelled() => break,
                }
                if let Err(err) = requests.sync().await {
                    tracing::warn!("requests: looking at the libraries failed: {err:#}");
                }
            }
        });
    }
}

/// Which of these items `user_id` may open.
async fn visible_items(db: &Db, user_id: i64, ids: &[i64]) -> sqlx::Result<HashSet<i64>> {
    if ids.is_empty() {
        return Ok(HashSet::new());
    }
    let sql = format!(
        "{ME_CTE} SELECT i.id FROM items i LEFT JOIN items p ON p.id = i.parent_id
           LEFT JOIN items gp ON gp.id = i.grandparent_id
         WHERE i.id IN ({}) AND {VISIBLE}",
        vec!["?"; ids.len()].join(",")
    );
    let mut query = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql.as_str())).bind(user_id);
    for id in ids {
        query = query.bind(id);
    }
    Ok(query.fetch_all(&db.read).await?.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn season(number: i64, aired: i64) -> discover::Season {
        discover::Season { number, name: String::new(), episodes: aired, aired, air_date: None, poster_path: None }
    }

    #[test]
    fn a_season_is_whole_with_every_episode_aired() {
        let counts = HashMap::from([(1, (10, 10)), (2, (4, 10)), (3, (3, 5))]);
        assert_eq!(whole(&counts, &season(1, 10)), Whole::Yes);
        assert_eq!(whole(&counts, &season(1, 8)), Whole::Yes, "more than aired");
        assert_eq!(whole(&counts, &season(2, 10)), Whole::Maybe, "the last one there: files of two episodes?");
        assert_eq!(whole(&counts, &season(3, 6)), Whole::No);
        assert_eq!(whole(&counts, &season(4, 0)), Whole::No, "nothing aired yet");
        assert_eq!(whole(&counts, &season(5, 3)), Whole::No);
    }

    #[test]
    fn limits_say_when_they_free_up() {
        let at = crate::util::date_secs("2026-11-03").unwrap() + 14 * 3600 + 5 * 60;
        let quota =
            Quota { movies_left: Some(0), seasons_left: Some(2), movies_free_at: Some(at), seasons_free_at: None };
        assert_eq!((quota.next_at(TitleKind::Movie), quota.next_at(TitleKind::Show)), (Some(at), None));
        let refused = quota.allows(TitleKind::Movie, 1).unwrap_err().to_string();
        assert_eq!(
            refused,
            "this account has asked for all the movies it may in 7 days: it may ask again from 2026-11-03 14:05 UTC"
        );
        assert!(quota.allows(TitleKind::Show, 2).is_ok());
        assert_eq!(
            quota.allows(TitleKind::Show, 3).unwrap_err().to_string(),
            "this account may ask for 2 more seasons for now"
        );
        let none = Quota::default();
        assert!(none.allows(TitleKind::Movie, 100).is_ok() && none.next_at(TitleKind::Movie).is_none(), "no limit");
    }

    #[test]
    fn access_and_statuses_read_as_stored() {
        for access in [RequestAccess::Off, RequestAccess::Ask, RequestAccess::Auto] {
            assert_eq!(RequestAccess::parse(access.as_str()), access);
        }
        assert_eq!(RequestAccess::parse("later"), RequestAccess::Ask);
        for status in RequestStatus::ALL {
            assert_eq!(RequestStatus::parse(status.as_str()), Some(status));
            assert_eq!(serde_json::to_value(status).unwrap(), json!(status.as_str()));
        }
        assert_eq!(RequestStatus::parse("lost"), None);
    }
}
