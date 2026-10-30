//! Requests and Discover, against local stand-ins for TMDB, Radarr and
//! Sonarr that answer like their APIs for a handful of titles and keep what
//! they were sent. Nothing leaves the machine.

mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{BodyDataStream, Bytes};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use parking_lot::Mutex;
use pley::events::ServerEvent;
use serde_json::{Value, json};

use common::TestApp;

const KEY: &str = "test-key";
const RADARR_KEY: &str = "radarr-key";
const SONARR_KEY: &str = "sonarr-key";

/// What one of the services was sent.
#[derive(Debug, Clone)]
struct Call {
    method: String,
    /// Without the service's prefix ("/movie", "/series/lookup").
    path: String,
    query: Vec<(String, String)>,
    /// `X-Api-Key`.
    key: Option<String>,
    body: Value,
}

#[derive(Clone, Default)]
struct Services {
    radarr: Arc<Mutex<Vec<Call>>>,
    sonarr: Arc<Mutex<Vec<Call>>>,
    tmdb: Arc<Mutex<Vec<String>>>,
    movies: Arc<Mutex<Vec<Value>>>,
    series: Arc<Mutex<Vec<Value>>>,
    /// Radarr refuses this many movies added before it takes one.
    refusals: Arc<Mutex<u32>>,
}

impl Services {
    fn radarr_calls(&self, method: &str, path: &str) -> Vec<Call> {
        self.radarr.lock().iter().filter(|c| c.method == method && c.path == path).cloned().collect()
    }

    fn sonarr_calls(&self, method: &str, path: &str) -> Vec<Call> {
        self.sonarr.lock().iter().filter(|c| c.method == method && c.path == path).cloned().collect()
    }
}

fn movie_details(id: i64) -> Option<Value> {
    let (title, date, imdb) = match id {
        603 => ("The Matrix", "1999-03-31", "tt0133093"),
        604 => ("The Matrix Reloaded", "2003-05-15", "tt0234215"),
        605 => ("The Matrix Revolutions", "2003-11-05", "tt0242653"),
        _ => return None,
    };
    Some(json!({
        "id": id, "title": title, "original_title": title, "original_language": "en", "overview": "Neo.",
        "tagline": "Welcome to the Real World.", "release_date": date, "runtime": 136,
        "genres": [{"id": 28, "name": "Action"}, {"id": 878, "name": "Science Fiction"}],
        "production_companies": [{"name": "Village Roadshow Pictures"}],
        "poster_path": "/matrix.jpg", "backdrop_path": "/matrix-wide.jpg",
        "vote_average": 8.2, "vote_count": 25000, "imdb_id": imdb,
        "keywords": {"keywords": [{"id": 310, "name": "artificial intelligence"}]},
        "release_dates": {"results": [{"iso_3166_1": "US", "release_dates": [{"certification": ""}, {"certification": "R"}]}]},
        "credits": {"cast": [{"name": "Keanu Reeves", "character": "Neo", "profile_path": "/keanu.jpg"}]}
    }))
}

fn tmdb_answer(path: &str, query: &[(String, String)]) -> Option<Value> {
    let param = |name: &str| query.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
    Some(match path {
        "/trending/all/week" => json!({"page": 1, "total_pages": 3, "results": [
            {"id": 603, "media_type": "movie", "title": "The Matrix", "release_date": "1999-03-31",
             "vote_average": 8.2, "vote_count": 25000, "poster_path": "/matrix.jpg", "overview": "Neo."},
            {"id": 1396, "media_type": "tv", "name": "Breaking Bad", "first_air_date": "2008-01-20",
             "vote_average": 8.9, "vote_count": 14000},
            {"id": 287, "media_type": "person", "name": "Brad Pitt"}
        ]}),
        "/discover/tv" if param("with_keywords") == Some("210024") => json!({"page": 1, "total_pages": 1, "results": [
            {"id": 9001, "name": "Space Cowboys", "first_air_date": "1998-04-03", "vote_average": 8.5, "vote_count": 3000}
        ]}),
        "/search/multi" => json!({"page": 1, "total_pages": 1, "results": match param("query") {
            Some("matrix") => vec![
                json!({"id": 603, "media_type": "movie", "title": "The Matrix", "release_date": "1999-03-31"}),
                json!({"id": 604, "media_type": "movie", "title": "The Matrix Reloaded", "release_date": "2003-05-15"}),
            ],
            _ => vec![],
        }}),
        "/search/movie" | "/search/tv" => json!({"results": []}),
        "/tv/1396" => json!({
            "name": "Breaking Bad", "original_name": "Breaking Bad", "original_language": "en", "origin_country": ["US"],
            "overview": "A chemistry teacher.", "first_air_date": "2008-01-20", "status": "Ended",
            "genres": [{"id": 18, "name": "Drama"}], "networks": [{"name": "AMC"}],
            "poster_path": "/bb.jpg", "backdrop_path": null, "vote_average": 8.9, "vote_count": 14000,
            "seasons": [
                {"season_number": 0, "name": "Specials", "episode_count": 2, "air_date": "2009-02-17"},
                {"season_number": 1, "name": "Season 1", "episode_count": 3, "air_date": "2008-01-20"},
                {"season_number": 2, "name": "Season 2", "episode_count": 2, "air_date": "2009-03-08"}
            ],
            "last_episode_to_air": {"season_number": 2, "episode_number": 2},
            "keywords": {"results": []},
            "content_ratings": {"results": [{"iso_3166_1": "US", "rating": "TV-MA"}]},
            "external_ids": {"imdb_id": "tt0903747", "tvdb_id": 81189},
            "aggregate_credits": {"cast": [{"name": "Bryan Cranston", "roles": [{"character": "Walter White"}]}]}
        }),
        "/tv/1396/external_ids" => json!({"imdb_id": "tt0903747", "tvdb_id": 81189}),
        "/tv/9001" => json!({
            "name": "Space Cowboys", "original_name": "Uchuu no Kaubōi", "original_language": "ja",
            "origin_country": ["JP"], "overview": "Bounty hunters.", "first_air_date": "1998-04-03",
            "status": "Ended", "genres": [{"id": 16, "name": "Animation"}], "networks": [{"name": "TV Tokyo"}],
            "seasons": [
                {"season_number": 1, "episode_count": 2, "air_date": "1998-04-03"},
                {"season_number": 2, "episode_count": 2, "air_date": "1999-04-03"},
                {"season_number": 3, "episode_count": 2, "air_date": "2000-04-03"}
            ],
            "last_episode_to_air": {"season_number": 3, "episode_number": 2},
            "keywords": {"results": [{"id": 210024, "name": "anime"}]},
            "external_ids": {"imdb_id": null, "tvdb_id": 76885}
        }),
        "/tv/9001/external_ids" => json!({"imdb_id": null, "tvdb_id": 76885}),
        path => movie_details(path.strip_prefix("/movie/")?.parse().ok()?)?,
    })
}

fn radarr_answer(s: &Services, call: &Call) -> (StatusCode, Value) {
    let param = |name: &str| call.query.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
    let tmdb = || param("tmdbId").and_then(|t| t.parse::<i64>().ok());
    match (call.method.as_str(), call.path.as_str()) {
        ("GET", "/system/status") => (StatusCode::OK, json!({"version": "5.14.0.9383"})),
        ("GET", "/qualityprofile") => {
            (StatusCode::OK, json!([{"id": 4, "name": "HD-1080p"}, {"id": 5, "name": "Ultra-HD"}]))
        }
        ("GET", "/rootfolder") => (StatusCode::OK, json!([{"path": "/movies", "freeSpace": 5_000_000_000_i64}])),
        ("GET", "/movie") => {
            let found: Vec<Value> =
                s.movies.lock().iter().filter(|m| m["tmdbId"].as_i64() == tmdb()).cloned().collect();
            (StatusCode::OK, json!(found))
        }
        ("GET", "/movie/lookup/tmdb") => match tmdb().and_then(movie_details) {
            Some(m) => (StatusCode::OK, json!({"title": m["title"], "tmdbId": tmdb(), "year": 1999, "images": []})),
            None => (StatusCode::NOT_FOUND, json!({"message": "NotFound"})),
        },
        ("POST", "/movie") => {
            let mut refusals = s.refusals.lock();
            if *refusals > 0 {
                *refusals -= 1;
                let errors = json!([{"propertyName": "RootFolderPath", "errorMessage": "Root folder is not writable"}]);
                return (StatusCode::BAD_REQUEST, errors);
            }
            let mut movies = s.movies.lock();
            let mut movie = call.body.clone();
            movie["id"] = json!(11 + movies.len());
            movie["hasFile"] = json!(false);
            movies.push(movie.clone());
            (StatusCode::CREATED, movie)
        }
        ("PUT", path) if path.starts_with("/movie/") => (StatusCode::ACCEPTED, call.body.clone()),
        ("POST", "/command") => (StatusCode::CREATED, json!({"id": 1, "name": call.body["name"]})),
        ("GET", "/queue") => (
            StatusCode::OK,
            json!({"records": [
                {"movieId": 11, "size": 1000.0, "sizeleft": 400.0, "status": "downloading",
                 "trackedDownloadState": "downloading", "trackedDownloadStatus": "ok",
                 "estimatedCompletionTime": "2030-01-01T00:00:00Z"}
            ]}),
        ),
        _ => (StatusCode::NOT_FOUND, json!({"message": "NotFound"})),
    }
}

fn sonarr_answer(s: &Services, call: &Call) -> (StatusCode, Value) {
    let param = |name: &str| call.query.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
    match (call.method.as_str(), call.path.as_str()) {
        ("GET", "/system/status") => (StatusCode::OK, json!({"version": "4.0.11.2680"})),
        ("GET", "/qualityprofile") => (StatusCode::OK, json!([{"id": 6, "name": "HD-720p"}])),
        ("GET", "/rootfolder") => (StatusCode::OK, json!([{"path": "/tv", "freeSpace": null}, {"path": "/anime"}])),
        ("GET", "/series") => {
            let tvdb = param("tvdbId").and_then(|t| t.parse::<i64>().ok());
            let found: Vec<Value> = s.series.lock().iter().filter(|m| m["tvdbId"].as_i64() == tvdb).cloned().collect();
            (StatusCode::OK, json!(found))
        }
        ("GET", "/series/lookup") => {
            let tvdb = param("term").and_then(|t| t.strip_prefix("tvdb:")?.parse::<i64>().ok());
            let seasons = |n: i64| (0..=n).map(|s| json!({"seasonNumber": s, "monitored": true})).collect::<Vec<_>>();
            let found = match tvdb {
                Some(81189) => vec![json!({"title": "Breaking Bad", "tvdbId": 81189, "seasons": seasons(2)})],
                Some(76885) => vec![json!({"title": "Space Cowboys", "tvdbId": 76885, "seasons": seasons(3)})],
                _ => vec![],
            };
            (StatusCode::OK, json!(found))
        }
        ("POST", "/series") => {
            let mut series = s.series.lock();
            let mut show = call.body.clone();
            show["id"] = json!(21 + series.len());
            series.push(show.clone());
            (StatusCode::CREATED, show)
        }
        ("PUT", path) if path.starts_with("/series/") => (StatusCode::ACCEPTED, call.body.clone()),
        ("POST", "/command") => (StatusCode::CREATED, json!({"id": 2, "name": call.body["name"]})),
        ("GET", "/queue") => (
            StatusCode::OK,
            json!({"records": [
                {"seriesId": 21, "seasonNumber": 2, "size": 100.0, "sizeleft": 100.0, "status": "queued"},
                {"seriesId": 21, "seasonNumber": 1, "size": 100.0, "sizeleft": 0.0, "status": "completed",
                 "trackedDownloadState": "importPending"}
            ]}),
        ),
        // Sonarr 4's: no language profiles.
        _ => (StatusCode::NOT_FOUND, json!({"message": "NotFound"})),
    }
}

async fn serve(
    State(s): State<Services>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    Query(query): Query<Vec<(String, String)>>,
    body: Bytes,
) -> Response {
    let path = uri.path();
    if let Some(path) = path.strip_prefix("/tmdb") {
        s.tmdb.lock().push(format!("{path}?{}", uri.query().unwrap_or_default()));
        if !query.iter().any(|(k, v)| k == "api_key" && v == KEY) {
            return (StatusCode::UNAUTHORIZED, axum::Json(json!({"status_code": 7}))).into_response();
        }
        return match tmdb_answer(path, &query) {
            Some(body) => axum::Json(body).into_response(),
            None => (StatusCode::NOT_FOUND, axum::Json(json!({"status_code": 34}))).into_response(),
        };
    }
    let (service, key, path) = match (path.strip_prefix("/radarr/api/v3"), path.strip_prefix("/sonarr/api/v3")) {
        (Some(path), _) => ("radarr", RADARR_KEY, path),
        (_, Some(path)) => ("sonarr", SONARR_KEY, path),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let call = Call {
        method: method.to_string(),
        path: path.to_owned(),
        query,
        key: headers.get("x-api-key").and_then(|k| k.to_str().ok()).map(str::to_owned),
        body: serde_json::from_slice(&body).unwrap_or(Value::Null),
    };
    let calls = if service == "radarr" { &s.radarr } else { &s.sonarr };
    calls.lock().push(call.clone());
    if call.key.as_deref() != Some(key) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    let (status, body) = if service == "radarr" { radarr_answer(&s, &call) } else { sonarr_answer(&s, &call) };
    (status, axum::Json(body)).into_response()
}

async fn start_services() -> (Services, String) {
    let services = Services::default();
    let app = Router::new().fallback(serve).with_state(services.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (services, format!("http://{addr}"))
}

struct World {
    app: TestApp,
    services: Services,
    base: String,
    admin: String,
    ann: String,
    ann_id: i64,
    movies: i64,
    shows: i64,
}

/// pley with TMDB at the stand-in (the other providers too; they are never
/// asked here), empty movie and show libraries, and Ann, who asks.
async fn world() -> World {
    let (services, base) = start_services().await;
    let app = TestApp::with_config(|c| {
        c.metadata_endpoints = Some(pley::providers::Endpoints {
            tvmaze: format!("{base}/tvmaze"),
            tvdb: format!("{base}/tvdb"),
            musicbrainz: format!("{base}/mb"),
            cover_art: format!("{base}/caa"),
            tmdb: format!("{base}/tmdb"),
            opensubtitles: format!("{base}/opensubtitles"),
        });
    })
    .await;
    let admin = app.setup_admin().await;
    let movies = app.library(&admin, "Movies", "movies", "movies").await;
    let shows = app.library(&admin, "Shows", "shows", "shows").await;
    let (ann, ann_id) = app.user(&admin, "ann", json!({})).await;
    World { app, services, base, admin, ann, ann_id, movies, shows }
}

impl World {
    /// Requests on, with Radarr and Sonarr at the stand-in; `more` changes
    /// some of the settings.
    async fn turn_on(&self, more: Value) {
        let mut settings = json!({
            "tmdb_api_key": KEY,
            "requests": true,
            "radarr_url": format!("{}/radarr", self.base),
            "radarr_api_key": RADARR_KEY,
            "radarr_quality_profile": 4,
            "radarr_root_folder": "/movies",
            "sonarr_url": format!("{}/sonarr", self.base),
            "sonarr_api_key": SONARR_KEY,
            "sonarr_quality_profile": 6,
            "sonarr_root_folder": "/tv",
            "sonarr_anime_root_folder": "/anime",
        });
        for (key, value) in more.as_object().unwrap() {
            settings[key] = value.clone();
        }
        let reply = self.app.put("/api/settings", &self.admin, settings).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
        self.app.wait_for_scans().await;
    }

    /// Files added to a library, scanned.
    async fn add(&self, library: i64, files: &[&str]) {
        for file in files {
            self.app.media_file(file, 1024);
        }
        let reply =
            self.app.post(&format!("/api/libraries/{library}/scan"), &self.admin, json!({"force": false})).await;
        assert_eq!(reply.status, StatusCode::ACCEPTED, "{}", reply.text());
        self.app.wait_for_scans().await;
    }

    /// Gives the library's movie or show `title` ids, as metadata would:
    /// `set` is SQL (`tmdb_id = 603`).
    async fn identify(&self, title: &str, set: &str) {
        let done = sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE items SET {set} WHERE title = ? AND kind IN ('movie', 'show')"
        )))
        .bind(title)
        .execute(&self.app.state.db.write)
        .await
        .unwrap();
        assert_eq!(done.rows_affected(), 1, "{title}");
    }

    async fn ask(&self, token: &str, body: Value) -> common::Reply {
        self.app.post("/api/requests", token, body).await
    }
}

fn status(reply: &common::Reply) -> (StatusCode, String) {
    (reply.status, reply.body["status"].as_str().unwrap_or_default().to_owned())
}

#[tokio::test]
async fn requests_stay_off_until_turned_on() {
    let w = world().await;
    let summary = w.app.get("/api/requests/summary", &w.ann).await;
    assert_eq!(summary.status, StatusCode::OK);
    assert_eq!(summary.body["enabled"], json!(false));
    assert_eq!(summary.body["access"], json!("ask"));
    for uri in ["/api/discover/trending", "/api/discover/search?q=matrix", "/api/requests", "/api/discover/movie/603"] {
        let off = w.app.get(uri, &w.ann).await;
        assert_eq!(off.status, StatusCode::SERVICE_UNAVAILABLE, "{uri}");
        assert_eq!((&off.body["code"], &off.body["setup"]), (&json!("unavailable"), &json!(false)), "{uri}");
    }
    // They find titles on TMDB.
    let refused = w.app.put("/api/settings", &w.admin, json!({"requests": true})).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    w.turn_on(json!({})).await;
    let summary = w.app.get("/api/requests/summary", &w.admin).await;
    assert_eq!(
        summary.body,
        json!({"enabled": true, "access": "auto", "movies_left": null, "seasons_left": null,
               "movies_next_at": null, "seasons_next_at": null, "pending": 0})
    );
    // A key TMDB refuses: Discover says TMDB is the trouble.
    let reply = w.app.put("/api/settings", &w.admin, json!({"tmdb_api_key": "wrong"})).await;
    assert_eq!(reply.status, StatusCode::OK);
    w.app.wait_for_scans().await;
    let refused = w.app.get("/api/discover/trending", &w.ann).await;
    assert_eq!(refused.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(refused.body["setup"], json!(true), "{}", refused.text());
    assert!(refused.body["message"].as_str().unwrap().contains("TMDB rejected the API key"));
    // Nor are Radarr's and Sonarr's keys ever shown.
    let settings = w.app.get("/api/settings", &w.admin).await;
    assert!(!settings.mentions(RADARR_KEY) && !settings.mentions(SONARR_KEY), "{}", settings.text());
    assert_eq!(
        (&settings.body["radarr_api_key_set"], &settings.body["sonarr_api_key_set"]),
        (&json!(true), &json!(true))
    );
}

#[tokio::test]
async fn a_movie_is_asked_for_approved_sent_to_radarr_and_found() {
    let w = world().await;
    w.turn_on(json!({})).await;
    let mut events = w.app.state.events.subscribe();

    let trending = w.app.get("/api/discover/trending", &w.ann).await;
    assert_eq!(trending.status, StatusCode::OK, "{}", trending.text());
    let results = trending.body["results"].as_array().unwrap();
    assert_eq!(results.len(), 2, "people are left out: {results:?}");
    assert_eq!(results[0]["kind"], json!("movie"));
    assert_eq!(results[0]["status"], json!("none"));
    assert_eq!(results[0]["poster_url"], json!("https://image.tmdb.org/t/p/w342/matrix.jpg"));
    assert_eq!((results[0]["year"].as_i64(), results[0]["rating"].as_f64()), (Some(1999), Some(8.2)));
    assert_eq!((&results[1]["kind"], &results[1]["title"]), (&json!("show"), &json!("Breaking Bad")));
    assert_eq!(trending.body["total_pages"], json!(3));

    let asked = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 603})).await;
    assert_eq!(asked.status, StatusCode::CREATED, "{}", asked.text());
    assert_eq!(asked.body["status"], json!("pending"));
    assert_eq!(asked.body["requested_by"]["name"], json!("ann"));
    assert_eq!((&asked.body["title"], &asked.body["year"]), (&json!("The Matrix"), &json!(1999)));
    assert_eq!(asked.body["seasons"], Value::Null);
    let id = asked.body["id"].as_i64().unwrap();
    assert!(w.services.radarr_calls("POST", "/movie").is_empty(), "nothing goes to Radarr before it's approved");

    // Asked for already, by anyone.
    let again = w.ask(&w.admin, json!({"kind": "movie", "tmdb_id": 603})).await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{}", again.text());
    let unknown = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 999})).await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    let seasons = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 604, "seasons": [1]})).await;
    assert_eq!(seasons.status, StatusCode::BAD_REQUEST);

    let found = w.app.get("/api/discover/search?q=matrix", &w.ann).await;
    let results = found.body["results"].as_array().unwrap();
    assert_eq!((&results[0]["status"], &results[0]["request_id"]), (&json!("requested"), &json!(id)));
    assert_eq!(results[1]["status"], json!("none"));
    let summary = w.app.get("/api/requests/summary", &w.admin).await;
    assert_eq!(summary.body["pending"], json!(1));
    let heard: Vec<ServerEvent> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    assert!(heard.iter().any(|e| matches!(e, ServerEvent::RequestsChanged { user_id } if *user_id == w.ann_id)));

    // Only administrators decide.
    let denied = w.app.post(&format!("/api/requests/{id}/approve"), &w.ann, json!({})).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    let approved = w.app.post(&format!("/api/requests/{id}/approve"), &w.admin, json!({})).await;
    assert_eq!(status(&approved), (StatusCode::OK, "approved".to_owned()), "{}", approved.text());
    assert_eq!(approved.body["decided_by"]["name"], json!("admin"));
    let added = w.services.radarr_calls("POST", "/movie");
    assert_eq!(added.len(), 1);
    let movie = &added[0].body;
    assert_eq!((&movie["tmdbId"], &movie["qualityProfileId"]), (&json!(603), &json!(4)));
    assert_eq!((&movie["rootFolderPath"], &movie["monitored"]), (&json!("/movies"), &json!(true)));
    assert_eq!(movie["minimumAvailability"], json!("released"));
    assert_eq!(movie["addOptions"], json!({"searchForMovie": true}));
    let again = w.app.post(&format!("/api/requests/{id}/approve"), &w.admin, json!({})).await;
    assert_eq!(again.status, StatusCode::CONFLICT, "approved already");

    // Radarr's queue says how far it is.
    let mine = w.app.get(&format!("/api/requests/{id}"), &w.ann).await;
    assert_eq!(
        mine.body["download"],
        json!({"progress": 0.6, "state": "downloading", "eta": 1_893_456_000, "episodes": 1})
    );
    let withdrawn = w.app.delete(&format!("/api/requests/{id}"), &w.ann).await;
    assert_eq!(withdrawn.status, StatusCode::CONFLICT, "approved: it stays");

    // The movie arrives.
    w.add(w.movies, &["movies/The Matrix (1999).mkv"]).await;
    w.identify("The Matrix", "tmdb_id = 603").await;
    w.app.state.requests.sync().await.unwrap();
    let item = w.app.ids(w.movies, &w.admin).await["The Matrix"];
    let mine = w.app.get(&format!("/api/requests/{id}"), &w.ann).await;
    assert_eq!(status(&mine), (StatusCode::OK, "available".to_owned()), "{}", mine.text());
    assert_eq!((&mine.body["item_id"], &mine.body["download"]), (&json!(item), &Value::Null));
    assert!(mine.body["available_at"].is_i64());
    let heard: Vec<ServerEvent> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    assert!(heard.iter().any(|e| matches!(e,
        ServerEvent::RequestAvailable { user_id, request_id, item_id, .. }
            if *user_id == w.ann_id && *request_id == id && *item_id == Some(item))));
    let trending = w.app.get("/api/discover/trending", &w.ann).await;
    let matrix = &trending.body["results"][0];
    assert_eq!((&matrix["status"], &matrix["item_id"]), (&json!("library"), &json!(item)), "{matrix}");
    let more = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 603})).await;
    assert_eq!(more.status, StatusCode::CONFLICT, "a library has it");

    let list = w.app.get("/api/requests", &w.ann).await;
    assert_eq!((&list.body["total"], &list.body["counts"]["available"]), (&json!(1), &json!(1)));
    assert_eq!(list.body["counts"]["pending"], json!(0));
}

#[tokio::test]
async fn a_show_is_asked_for_the_seasons_a_library_lacks() {
    let w = world().await;
    w.turn_on(json!({})).await;
    // Season 1 whole, found by TheTVDB's id (another TV source named it).
    w.add(
        w.shows,
        &[
            "shows/Breaking Bad (2008)/Season 01/Breaking Bad - S01E01.mkv",
            "shows/Breaking Bad (2008)/Season 01/Breaking Bad - S01E02.mkv",
            "shows/Breaking Bad (2008)/Season 01/Breaking Bad - S01E03.mkv",
        ],
    )
    .await;
    w.identify("Breaking Bad", "tvdb_id = 81189, tmdb_id = NULL").await;

    let detail = w.app.get("/api/discover/show/1396", &w.ann).await;
    assert_eq!(detail.status, StatusCode::OK, "{}", detail.text());
    assert_eq!(detail.body["status"], json!("library"));
    assert_eq!(detail.body["certification"], json!("TV-MA"));
    assert_eq!(detail.body["poster_url"], json!("https://image.tmdb.org/t/p/w500/bb.jpg"));
    let seasons: Vec<(i64, i64, i64)> = detail.body["seasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["number"].as_i64().unwrap(), s["aired"].as_i64().unwrap(), s["in_library"].as_i64().unwrap()))
        .collect();
    assert_eq!(seasons, [(0, 2, 0), (1, 3, 3), (2, 2, 0)]);
    let trending = w.app.get("/api/discover/trending", &w.ann).await;
    assert_eq!(trending.body["results"][1]["status"], json!("library"), "by TheTVDB's id");

    let unknown = w.ask(&w.ann, json!({"kind": "show", "tmdb_id": 1396, "seasons": [5]})).await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
    let whole = w.ask(&w.ann, json!({"kind": "show", "tmdb_id": 1396, "seasons": [1]})).await;
    assert_eq!(whole.status, StatusCode::CONFLICT, "a library has it whole");
    // Every season but specials, less the one there.
    let asked = w.ask(&w.ann, json!({"kind": "show", "tmdb_id": 1396})).await;
    assert_eq!(asked.status, StatusCode::CREATED, "{}", asked.text());
    assert_eq!(asked.body["seasons"], json!([2]));
    let id = asked.body["id"].as_i64().unwrap();
    let twice = w.ask(&w.admin, json!({"kind": "show", "tmdb_id": 1396, "seasons": [1, 2]})).await;
    assert_eq!(twice.status, StatusCode::CONFLICT, "asked for already, or there");
    let detail = w.app.get("/api/discover/show/1396", &w.admin).await;
    let requested: Vec<bool> =
        detail.body["seasons"].as_array().unwrap().iter().map(|s| s["requested"].as_bool().unwrap()).collect();
    assert_eq!(requested, [false, false, true]);
    assert_eq!(detail.body["requests"][0]["id"], json!(id), "administrators see everyone's");

    let approved = w.app.post(&format!("/api/requests/{id}/approve"), &w.admin, json!({})).await;
    assert_eq!(status(&approved), (StatusCode::OK, "approved".to_owned()), "{}", approved.text());
    let added = w.services.sonarr_calls("POST", "/series");
    assert_eq!(added.len(), 1);
    let series = &added[0].body;
    assert_eq!((&series["tvdbId"], &series["qualityProfileId"]), (&json!(81189), &json!(6)));
    assert_eq!((&series["rootFolderPath"], &series["seriesType"]), (&json!("/tv"), &json!("standard")));
    assert_eq!((&series["seasonFolder"], &series["monitored"]), (&json!(true), &json!(true)));
    let monitored: Vec<bool> =
        series["seasons"].as_array().unwrap().iter().map(|s| s["monitored"].as_bool().unwrap()).collect();
    assert_eq!(monitored, [false, false, true], "the season asked for only");
    assert_eq!(series["addOptions"]["searchForMissingEpisodes"], json!(true));
    assert!(series.get("languageProfileId").is_none(), "Sonarr 4 has none");
    // Its queue, for the seasons asked for.
    let mine = w.app.get(&format!("/api/requests/{id}"), &w.ann).await;
    assert_eq!(mine.body["download"], json!({"progress": 0.0, "state": "queued", "eta": null, "episodes": 1}));

    // One episode of season 2, then both.
    w.add(w.shows, &["shows/Breaking Bad (2008)/Season 02/Breaking Bad - S02E01.mkv"]).await;
    w.app.state.requests.sync().await.unwrap();
    let mine = w.app.get(&format!("/api/requests/{id}"), &w.ann).await;
    assert_eq!(status(&mine), (StatusCode::OK, "approved".to_owned()));
    assert_eq!(mine.body["available_seasons"], json!([]));
    w.add(w.shows, &["shows/Breaking Bad (2008)/Season 02/Breaking Bad - S02E02.mkv"]).await;
    w.app.state.requests.sync().await.unwrap();
    let mine = w.app.get(&format!("/api/requests/{id}"), &w.ann).await;
    assert_eq!(status(&mine), (StatusCode::OK, "available".to_owned()), "{}", mine.text());
    assert_eq!(mine.body["available_seasons"], json!([2]));
    assert!(mine.body["item_id"].is_i64());
}

#[tokio::test]
async fn anime_goes_to_its_own_folder_and_limits_count_seasons() {
    let w = world().await;
    w.turn_on(json!({"request_limit_seasons": 2, "request_limit_movies": 1})).await;
    let user = format!("/api/users/{}", w.ann_id);
    let reply = w.app.patch(&user, &w.admin, json!({"request_access": "auto"})).await;
    assert_eq!((reply.status, &reply.body["request_access"]), (StatusCode::OK, &json!("auto")));

    let anime = w.app.get("/api/discover/anime", &w.ann).await;
    assert_eq!(anime.body["results"][0]["title"], json!("Space Cowboys"), "{}", anime.text());
    let detail = w.app.get("/api/discover/show/9001", &w.ann).await;
    assert_eq!((&detail.body["anime"], &detail.body["original_title"]), (&json!(true), &json!("Uchuu no Kaubōi")));

    let too_many = w.ask(&w.ann, json!({"kind": "show", "tmdb_id": 9001})).await;
    assert_eq!(too_many.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(too_many.body["message"], json!("this account may ask for 2 more seasons for now"));
    // Approved as asked, to anime's own folder.
    let asked = w.ask(&w.ann, json!({"kind": "show", "tmdb_id": 9001, "seasons": [2, 1]})).await;
    assert_eq!(status(&asked), (StatusCode::CREATED, "approved".to_owned()), "{}", asked.text());
    assert_eq!((&asked.body["seasons"], &asked.body["anime"]), (&json!([1, 2]), &json!(true)));
    assert_eq!(asked.body["decided_by"]["name"], json!("ann"));
    let series = &w.services.sonarr_calls("POST", "/series")[0].body;
    assert_eq!((&series["rootFolderPath"], &series["seriesType"]), (&json!("/anime"), &json!("anime")));

    let none_left = w.ask(&w.ann, json!({"kind": "show", "tmdb_id": 9001, "seasons": [3]})).await;
    assert_eq!(none_left.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(none_left.body["message"].as_str().unwrap().starts_with("this account has asked for all the seasons"));
    let summary = w.app.get("/api/requests/summary", &w.ann).await;
    assert_eq!((&summary.body["access"], &summary.body["seasons_left"]), (&json!("auto"), &json!(0)));
    assert_eq!(summary.body["movies_left"], json!(1));
    let made = asked.body["created_at"].as_i64().unwrap();
    assert_eq!(summary.body["seasons_next_at"], json!(made + 7 * 86_400));
    assert_eq!(summary.body["movies_next_at"], Value::Null, "movies are left");
    // Administrators have no limit, and a show there already gets the
    // seasons asked for monitored and searched for.
    let more = w.ask(&w.admin, json!({"kind": "show", "tmdb_id": 9001, "seasons": [3]})).await;
    assert_eq!(status(&more), (StatusCode::CREATED, "approved".to_owned()), "{}", more.text());
    assert_eq!(w.services.sonarr_calls("POST", "/series").len(), 1);
    let put = w.services.sonarr_calls("PUT", "/series/21");
    let monitored: Vec<bool> =
        put[0].body["seasons"].as_array().unwrap().iter().map(|s| s["monitored"].as_bool().unwrap()).collect();
    assert_eq!(monitored, [false, true, true, true]);
    let searches = w.services.sonarr_calls("POST", "/command");
    assert_eq!(searches.last().unwrap().body, json!({"name": "SeasonSearch", "seriesId": 21, "seasonNumber": 3}));
}

#[tokio::test]
async fn failures_say_why_and_approving_again_retries() {
    let w = world().await;
    w.turn_on(json!({})).await;
    *w.services.refusals.lock() = 1;
    let asked = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 604})).await;
    let id = asked.body["id"].as_i64().unwrap();
    let failed = w.app.post(&format!("/api/requests/{id}/approve"), &w.admin, json!({})).await;
    assert_eq!(status(&failed), (StatusCode::OK, "failed".to_owned()));
    assert_eq!(failed.body["reason"], json!("Radarr refused: Root folder is not writable"));
    // Again, with another profile and folder.
    let approved = w
        .app
        .post(&format!("/api/requests/{id}/approve"), &w.admin, json!({"quality_profile": 5, "root_folder": " /4k "}))
        .await;
    assert_eq!(status(&approved), (StatusCode::OK, "approved".to_owned()), "{}", approved.text());
    assert_eq!(approved.body["reason"], Value::Null);
    let movie = &w.services.radarr_calls("POST", "/movie")[1].body;
    assert_eq!((&movie["qualityProfileId"], &movie["rootFolderPath"]), (&json!(5), &json!("/4k")));

    // A key Radarr refuses.
    let reply = w.app.put("/api/settings", &w.admin, json!({"radarr_api_key": "wrong"})).await;
    assert_eq!(reply.status, StatusCode::OK);
    let asked = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 605})).await;
    let id = asked.body["id"].as_i64().unwrap();
    let failed = w.app.post(&format!("/api/requests/{id}/approve"), &w.admin, json!({})).await;
    assert_eq!(failed.body["reason"], json!("Radarr rejected the API key"));
    // Without Radarr, approving is all there is.
    let reply = w.app.put("/api/settings", &w.admin, json!({"radarr_url": ""})).await;
    assert_eq!(reply.status, StatusCode::OK);
    let approved = w.app.post(&format!("/api/requests/{id}/approve"), &w.admin, json!({})).await;
    assert_eq!(status(&approved), (StatusCode::OK, "approved".to_owned()));
    assert_eq!(approved.body["download"], Value::Null);
}

#[tokio::test]
async fn requests_are_declined_withdrawn_and_kept_to_their_makers() {
    let w = world().await;
    w.turn_on(json!({"request_limit_movies": 1})).await;
    let (bob, _) = w.app.user(&w.admin, "bob", json!({})).await;
    let asked = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 604})).await;
    let id = asked.body["id"].as_i64().unwrap();

    // Others' requests aren't there for Bob.
    assert_eq!(w.app.get(&format!("/api/requests/{id}"), &bob).await.status, StatusCode::NOT_FOUND);
    assert_eq!(w.app.delete(&format!("/api/requests/{id}"), &bob).await.status, StatusCode::NOT_FOUND);
    let list = w.app.get("/api/requests", &bob).await;
    assert_eq!((&list.body["total"], &list.body["requests"]), (&json!(0), &json!([])));
    let search = w.app.get("/api/discover/search?q=matrix", &bob).await;
    let reloaded = &search.body["results"][1];
    assert_eq!((&reloaded["status"], &reloaded["request_id"]), (&json!("requested"), &Value::Null));

    let long = "x".repeat(501);
    let refused = w.app.post(&format!("/api/requests/{id}/decline"), &w.admin, json!({"reason": long})).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    let declined =
        w.app.post(&format!("/api/requests/{id}/decline"), &w.admin, json!({"reason": " Not this one "})).await;
    assert_eq!(status(&declined), (StatusCode::OK, "declined".to_owned()));
    assert_eq!(declined.body["reason"], json!("Not this one"));
    let again = w.app.post(&format!("/api/requests/{id}/approve"), &w.admin, json!({})).await;
    assert_eq!(again.status, StatusCode::CONFLICT, "declined");
    // A declined request doesn't count.
    let summary = w.app.get("/api/requests/summary", &w.ann).await;
    assert_eq!(summary.body["movies_left"], json!(1));
    let other = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 605})).await;
    assert_eq!(other.status, StatusCode::CREATED, "{}", other.text());
    let past = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 603})).await;
    assert_eq!(past.status, StatusCode::TOO_MANY_REQUESTS);

    let all = w.app.get("/api/requests?status=pending", &w.admin).await;
    assert_eq!((&all.body["total"], &all.body["counts"]["declined"]), (&json!(1), &json!(1)));
    assert_eq!(all.body["requests"][0]["id"], other.body["id"]);
    let mine = w.app.get("/api/requests?mine=1", &w.admin).await;
    assert_eq!(mine.body["total"], json!(0), "the administrator asked for nothing");
    assert_eq!(w.app.get("/api/requests?status=lost", &w.ann).await.status, StatusCode::BAD_REQUEST);

    // Ann withdraws hers.
    assert_eq!(w.app.delete(&format!("/api/requests/{id}"), &w.ann).await.status, StatusCode::NO_CONTENT);
    let other_id = other.body["id"].as_i64().unwrap();
    assert_eq!(w.app.delete(&format!("/api/requests/{other_id}"), &w.ann).await.status, StatusCode::NO_CONTENT);
    let list = w.app.get("/api/requests", &w.ann).await;
    assert_eq!(list.body["total"], json!(0));
}

#[tokio::test]
async fn who_may_ask() {
    let w = world().await;
    w.turn_on(json!({})).await;
    let kid = w.app.post("/api/users", &w.admin, json!({"username": "kid", "managed": true})).await;
    assert_eq!((kid.status, &kid.body["request_access"]), (StatusCode::CREATED, &json!("off")), "{}", kid.text());
    let (teen, _) = w.app.user(&w.admin, "teen", json!({"max_age": 13})).await;
    let summary = w.app.get("/api/requests/summary", &teen).await;
    assert_eq!((&summary.body["enabled"], &summary.body["access"]), (&json!(true), &json!("off")));
    for uri in ["/api/discover/trending", "/api/requests"] {
        assert_eq!(w.app.get(uri, &teen).await.status, StatusCode::FORBIDDEN, "{uri}");
    }
    assert_eq!(w.ask(&teen, json!({"kind": "movie", "tmdb_id": 603})).await.status, StatusCode::FORBIDDEN);

    let mut events = w.app.state.events.subscribe();
    let user = format!("/api/users/{}", w.ann_id);
    let reply = w.app.patch(&user, &w.admin, json!({"request_access": "off"})).await;
    assert_eq!(reply.body["request_access"], json!("off"));
    let heard: Vec<ServerEvent> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    assert!(heard.iter().any(|e| matches!(e, ServerEvent::RequestsChanged { user_id } if *user_id == w.ann_id)));
    assert_eq!(w.app.get("/api/discover/trending", &w.ann).await.status, StatusCode::FORBIDDEN);
    let wrong = w.app.patch(&user, &w.admin, json!({"request_access": "always"})).await;
    assert!(wrong.status.is_client_error());
}

/// `/api/events` as `token`'s account hears it.
async fn events(app: &TestApp, token: &str) -> BodyDataStream {
    let request = axum::http::Request::builder()
        .uri("/api/events")
        .header("authorization", format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(app.api.clone(), request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.into_body().into_data_stream()
}

/// What a stream says up to `libraries_changed`.
async fn heard(stream: &mut BodyDataStream) -> String {
    let mut received = String::new();
    let read = async {
        while let Some(chunk) = stream.next().await {
            received.push_str(&String::from_utf8_lossy(&chunk.unwrap()));
            if received.contains("libraries_changed") {
                return;
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(10), read).await.expect("the events");
    received
}

#[tokio::test]
async fn administrators_streams_hear_of_everyones_requests() {
    let w = world().await;
    w.turn_on(json!({})).await;
    let (bob, _) = w.app.user(&w.admin, "bob", json!({})).await;
    let mut admin = events(&w.app, &w.admin).await;
    let mut bob = events(&w.app, &bob).await;
    let asked = w.ask(&w.ann, json!({"kind": "movie", "tmdb_id": 603})).await;
    assert_eq!(asked.status, StatusCode::CREATED);
    w.app.state.events.send(ServerEvent::LibrariesChanged);
    let admin = heard(&mut admin).await;
    assert!(admin.contains(r#"{"type":"requests_changed"}"#), "{admin}");
    let bob = heard(&mut bob).await;
    assert!(!bob.contains("requests_changed"), "{bob}");
}

#[tokio::test]
async fn the_settings_ask_radarr_and_sonarr_what_they_offer() {
    let w = world().await;
    w.turn_on(json!({})).await;
    let options = w.app.post("/api/settings/arr", &w.admin, json!({"service": "radarr"})).await;
    assert_eq!(options.status, StatusCode::OK, "{}", options.text());
    assert_eq!(
        options.body,
        json!({
            "version": "5.14.0.9383",
            "quality_profiles": [{"id": 4, "name": "HD-1080p"}, {"id": 5, "name": "Ultra-HD"}],
            "root_folders": [{"path": "/movies", "free_bytes": 5_000_000_000_i64}],
            "language_profiles": []
        })
    );
    let sonarr = w
        .app
        .post(
            "/api/settings/arr",
            &w.admin,
            json!({"service": "sonarr", "url": format!("{}/sonarr/", w.base), "api_key": SONARR_KEY}),
        )
        .await;
    assert_eq!(sonarr.status, StatusCode::OK, "{}", sonarr.text());
    assert_eq!(
        sonarr.body["root_folders"],
        json!([{"path": "/tv", "free_bytes": null}, {"path": "/anime", "free_bytes": null}])
    );

    // The saved key goes to the saved address only.
    let elsewhere =
        w.app.post("/api/settings/arr", &w.admin, json!({"service": "radarr", "url": "http://127.0.0.2:9"})).await;
    assert_eq!(elsewhere.status, StatusCode::BAD_REQUEST);
    assert!(elsewhere.body["message"].as_str().unwrap().contains("saved address only"));
    let moved = w.app.put("/api/settings", &w.admin, json!({"radarr_url": "http://127.0.0.2:9"})).await;
    assert_eq!(moved.status, StatusCode::BAD_REQUEST, "{}", moved.text());
    let wrong = w.app.post("/api/settings/arr", &w.admin, json!({"service": "radarr", "api_key": "wrong"})).await;
    assert_eq!(
        (wrong.status, &wrong.body["message"]),
        (StatusCode::BAD_REQUEST, &json!("Radarr rejected the API key"))
    );
    let away = w
        .app
        .post("/api/settings/arr", &w.admin, json!({"service": "radarr", "url": "http://127.0.0.1:9", "api_key": "k"}))
        .await;
    assert_eq!(away.status, StatusCode::BAD_REQUEST);
    assert!(away.body["message"].as_str().unwrap().starts_with("Radarr couldn't be reached"), "{}", away.text());
    let bad = w.app.post("/api/settings/arr", &w.admin, json!({"service": "radarr", "url": "ftp://box"})).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        w.app.post("/api/settings/arr", &w.ann, json!({"service": "radarr"})).await.status,
        StatusCode::FORBIDDEN
    );
    // Each key went to its own service only.
    assert!(w.services.radarr.lock().iter().all(|c| c.key.as_deref() != Some(SONARR_KEY)));
    assert!(w.services.sonarr.lock().iter().all(|c| c.key.as_deref() != Some(RADARR_KEY)));
    assert!(w.services.tmdb.lock().iter().all(|r| !r.contains(RADARR_KEY) && !r.contains(SONARR_KEY)));
}
