//! Intro and credits detection end to end (docs/API.md, "Intro and credits
//! detection"): pley's background worker against a transcoder stand-in whose
//! fingerprints are noise, except for a theme tune the episodes share (at
//! different times) and the same last minute of end credits.

mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::{get, post};
use pley_transcoder_api::{
    API_VERSION, Capabilities, Chapter, Fingerprint, FingerprintRequest, MediaInfo, MediaStream, ProbeRequest,
    StreamKind, TranscoderInfo,
};
use serde_json::{Value, json};

use common::TestApp;

/// Chromaprint's step: 1365 samples at 11025 Hz.
const ITEM_SECONDS: f64 = 1365.0 / 11025.0;
const DURATION_MS: i64 = 8 * 60_000;
/// About 30 s of theme tune.
const THEME_ITEMS: usize = 242;
/// About the last minute: end credits.
const CREDITS_ITEMS: usize = 485;

fn item_ms() -> f64 {
    ITEM_SECONDS * 1000.0
}

/// Deterministic pseudo-random values (splitmix64).
fn noise(seed: u64) -> u32 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) as u32
}

/// Where each episode's theme starts, in fingerprint items.
fn theme_at(path: &str) -> usize {
    [("S01E01", 242), ("S01E02", 300), ("S01E03", 0), ("S02E01", 400)]
        .into_iter()
        .find(|(episode, _)| path.contains(episode))
        .map_or(usize::MAX / 2, |(_, at)| at)
}

fn fingerprint(req: &FingerprintRequest) -> Fingerprint {
    let first = (req.start_ms as f64 / item_ms()).round() as usize;
    let count = (req.duration_ms as f64 / item_ms()) as usize;
    let total = (DURATION_MS as f64 / item_ms()) as usize;
    let theme = theme_at(&req.path);
    let own = req.path.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    let data = (first..first + count)
        .map(|i| {
            if (theme..theme + THEME_ITEMS).contains(&i) {
                noise(1_000_000 + (i - theme) as u64)
            } else if i + CREDITS_ITEMS >= total {
                noise(2_000_000 + (i + CREDITS_ITEMS - total) as u64)
            } else {
                noise(own.wrapping_mul(1_000_003).wrapping_add(i as u64))
            }
        })
        .collect();
    Fingerprint { start_ms: req.start_ms, item_seconds: ITEM_SECONDS, data }
}

fn probe(path: &str) -> MediaInfo {
    let chapter = |start_ms, end_ms, title: &str| Chapter { start_ms, end_ms, title: Some(title.into()) };
    let chapters = if path.contains("S01E02") {
        vec![chapter(0, 37_000, "Cold Open"), chapter(37_000, 66_000, "Opening"), chapter(66_000, DURATION_MS, "Story")]
    } else if path.contains("Chaptered") {
        vec![
            chapter(0, 60_000, "Opening Credits"),
            chapter(60_000, DURATION_MS - 90_000, "Chapter 2"),
            chapter(DURATION_MS - 90_000, DURATION_MS, "End Credits"),
        ]
    } else {
        vec![]
    };
    MediaInfo {
        container: Some("mkv".into()),
        duration_ms: Some(DURATION_MS),
        streams: vec![
            MediaStream { index: 0, kind: StreamKind::Video, codec: "h264".into(), ..Default::default() },
            MediaStream { index: 1, kind: StreamKind::Audio, codec: "aac".into(), default: true, ..Default::default() },
        ],
        chapters,
        ..Default::default()
    }
}

async fn start_transcoder() -> (Arc<AtomicUsize>, String) {
    let fingerprints = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route(
            "/v1/info",
            get(|| async {
                Json(TranscoderInfo {
                    api_version: API_VERSION,
                    capabilities: Capabilities { ffmpeg_available: true, chromaprint: true, ..Default::default() },
                    ..Default::default()
                })
            }),
        )
        .route("/v1/probe", post(|Json(req): Json<ProbeRequest>| async move { Json(probe(&req.path)) }))
        .route(
            "/v1/fingerprint",
            post(|State(count): State<Arc<AtomicUsize>>, Json(req): Json<FingerprintRequest>| async move {
                count.fetch_add(1, Ordering::SeqCst);
                Json(fingerprint(&req))
            }),
        )
        .route("/v1/keyframes", post(|| async { axum::http::StatusCode::ACCEPTED }))
        .with_state(fingerprints.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (fingerprints, format!("http://{addr}"))
}

async fn world() -> (TestApp, Arc<AtomicUsize>, String) {
    let (fingerprints, base) = start_transcoder().await;
    let app = TestApp::with_config(|c| c.transcoder_url = Some(base)).await;
    app.state.transcoder.start();
    assert!(app.state.transcoder.wait_ready(Duration::from_secs(10)).await);
    let admin = app.setup_admin().await;
    app.put("/api/settings", &admin, json!({"seek_previews": false})).await;
    app.state.markers.start();
    (app, fingerprints, admin)
}

/// The markers of an item once the background worker stored `count` of them.
async fn markers(app: &TestApp, admin: &str, id: i64, count: usize) -> Vec<Value> {
    for _ in 0..500 {
        let item = app.get(&format!("/api/items/{id}"), admin).await;
        let markers = item.body["markers"].as_array().cloned().unwrap_or_default();
        if markers.len() >= count {
            return markers;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("item {id} did not get {count} markers");
}

/// `(kind, start, end, source)` of a marker, with the times rounded to seconds.
fn summary(marker: &Value) -> (String, i64, i64, String) {
    let seconds = |v: &Value| (v.as_i64().unwrap() as f64 / 1000.0).round() as i64;
    (
        marker["kind"].as_str().unwrap().to_owned(),
        seconds(&marker["start_ms"]),
        seconds(&marker["end_ms"]),
        marker["source"].as_str().unwrap().to_owned(),
    )
}

fn seconds(items: usize) -> i64 {
    (items as f64 * ITEM_SECONDS).round() as i64
}

#[tokio::test]
async fn shared_themes_and_credits_become_markers() {
    let (app, fingerprints, admin) = world().await;
    for (s, e) in [(1, 1), (1, 2), (1, 3), (2, 1)] {
        app.media_file(&format!("tv/Show (2020)/Season {s:02}/Show - S{s:02}E{e:02}.mkv"), 1024);
    }
    let tv = app.library(&admin, "TV", "shows", "tv").await;
    let show = app.ids(tv, &admin).await["Show"];
    let seasons = app.children(show, &admin).await;
    let first = app.children(seasons["Season 1"], &admin).await;
    let second = app.children(seasons["Season 2"], &admin).await;
    assert_eq!(
        app.post(&format!("/api/items/{show}/analyze"), &admin, json!({})).await.status,
        axum::http::StatusCode::ACCEPTED
    );

    let end = DURATION_MS / 1000;
    let credits_start = end - seconds(CREDITS_ITEMS);
    // The theme is found where each episode plays it (the cut is padded a
    // little at both ends); credits that reach the end run to the end.
    let e1 = markers(&app, &admin, first["Episode 1"], 2).await;
    let intro = summary(&e1[0]);
    assert_eq!((intro.0.as_str(), intro.3.as_str()), ("intro", "fingerprint"));
    assert!((intro.1 - seconds(242)).abs() <= 1, "{intro:?}");
    assert!((intro.2 - seconds(242 + THEME_ITEMS)).abs() <= 2, "{intro:?}");
    let found = summary(&e1[1]);
    assert!((found.1 - credits_start).abs() <= 1 && found.2 == end && found.3 == "fingerprint", "{found:?}");

    // A theme opening the episode starts at 0:00.
    let e3 = markers(&app, &admin, first["Episode 3"], 2).await;
    assert_eq!(summary(&e3[0]).1, 0);
    // Chapters win over fingerprints.
    let e2 = markers(&app, &admin, first["Episode 2"], 2).await;
    assert_eq!(summary(&e2[0]), ("intro".to_owned(), 37, 66, "chapter".to_owned()));
    assert_eq!(summary(&e2[1]).0, "credits");
    // A season of one episode is compared with the rest of the show.
    let lone = markers(&app, &admin, second["Episode 1"], 2).await;
    assert!((summary(&lone[0]).1 - seconds(400)).abs() <= 1, "{:?}", summary(&lone[0]));
    assert_eq!((summary(&lone[1]).0, summary(&lone[1]).2), ("credits".to_owned(), end));

    // Fingerprints are cached: analysing again asks the transcoder nothing.
    let asked = fingerprints.load(Ordering::SeqCst);
    assert!(asked > 0);
    sqlx::query("DELETE FROM markers").execute(&app.state.db.write).await.unwrap();
    app.post(&format!("/api/items/{}/analyze", first["Episode 1"]), &admin, json!({})).await;
    let again = markers(&app, &admin, first["Episode 1"], 2).await;
    assert_eq!(summary(&again[0]), intro);
    assert_eq!(fingerprints.load(Ordering::SeqCst), asked);
}

#[tokio::test]
async fn movies_use_their_chapters() {
    let (app, fingerprints, admin) = world().await;
    app.media_file("movies/Chaptered (2020).mkv", 1024);
    app.media_file("movies/Plain (2020).mkv", 1024);
    let movies = app.library(&admin, "Movies", "movies", "movies").await;
    let ids = app.ids(movies, &admin).await;
    for title in ["Chaptered", "Plain"] {
        let reply = app.post(&format!("/api/items/{}/analyze", ids[title]), &admin, json!({})).await;
        assert_eq!(reply.status, axum::http::StatusCode::ACCEPTED);
    }
    let found = markers(&app, &admin, ids["Chaptered"], 2).await;
    let end = DURATION_MS / 1000;
    assert_eq!(
        found.iter().map(summary).collect::<Vec<_>>(),
        [
            ("intro".to_owned(), 0, 60, "chapter".to_owned()),
            ("credits".to_owned(), end - 90, end, "chapter".to_owned())
        ]
    );
    let plain = app.get(&format!("/api/items/{}", ids["Plain"]), &admin).await;
    assert_eq!(plain.body["markers"], json!([]), "nothing to compare a movie with");
    assert_eq!(fingerprints.load(Ordering::SeqCst), 0, "movies are never fingerprinted");
}
