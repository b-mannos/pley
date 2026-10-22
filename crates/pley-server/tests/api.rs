//! End-to-end tests of the HTTP API, run in-process against a temporary data
//! directory. No transcoder is started, so scanned files stay unprobed (the
//! server must still list, browse, search and direct-play them).

use std::path::Path;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use pley::{AppState, Config};

struct TestApp {
    router: Router,
    state: AppState,
    media: tempfile::TempDir,
    _data: tempfile::TempDir,
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: Value,
    bytes: Vec<u8>,
}

impl TestApp {
    async fn new() -> Self {
        let data = tempfile::tempdir().unwrap();
        let mut config = Config::for_data_dir(data.path());
        config.transcoder_path = Some(data.path().join("no-transcoder-in-tests"));
        let state = AppState::init(config).await.unwrap();
        let router = pley::api::router(state.clone());
        Self { router, state, media: tempfile::tempdir().unwrap(), _data: data }
    }

    fn media_file(&self, rel: &str, bytes: usize) {
        let path = self.media.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7u8; bytes]).unwrap();
    }

    fn media_path(&self, rel: &str) -> String {
        self.media.path().join(rel).to_string_lossy().into_owned()
    }

    async fn call(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
        extra: &[(&str, &str)],
    ) -> Reply {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        for (k, v) in extra {
            req = req.header(*k, *v);
        }
        let req = match body {
            Some(b) => req.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())).unwrap(),
            None => req.body(Body::empty()).unwrap(),
        };
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024).await.unwrap().to_vec();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Reply { status, headers, body, bytes }
    }

    async fn get(&self, uri: &str, token: &str) -> Reply {
        self.call(Method::GET, uri, Some(token), None, &[]).await
    }

    async fn post(&self, uri: &str, token: &str, body: Value) -> Reply {
        self.call(Method::POST, uri, Some(token), Some(body), &[]).await
    }

    async fn setup_admin(&self) -> String {
        let r = self
            .call(Method::POST, "/api/setup", None, Some(json!({"username": "admin", "password": "secret1"})), &[])
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        r.body["token"].as_str().unwrap().to_owned()
    }

    async fn wait_for_scans(&self) {
        for _ in 0..200 {
            if self.state.scanner.statuses().is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("scan did not finish");
    }

    async fn library(&self, token: &str, name: &str, kind: &str, rel: &str) -> i64 {
        std::fs::create_dir_all(self.media.path().join(rel)).unwrap();
        let r = self
            .post("/api/libraries", token, json!({"name": name, "kind": kind, "paths": [self.media_path(rel)]}))
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        self.wait_for_scans().await;
        r.body["id"].as_i64().unwrap()
    }
}

fn titles(items: &Value) -> Vec<String> {
    items.as_array().unwrap().iter().map(|i| i["title"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test]
async fn setup_login_and_permissions() {
    let app = TestApp::new().await;
    let status = app.call(Method::GET, "/api/server", None, None, &[]).await;
    assert_eq!(status.body["setup_required"], json!(true));

    let admin = app.setup_admin().await;
    let again =
        app.call(Method::POST, "/api/setup", None, Some(json!({"username": "x", "password": "yyyyyy"})), &[]).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(app.call(Method::GET, "/api/server", None, None, &[]).await.body["setup_required"], json!(false));

    // Unauthenticated access is rejected, bad passwords too.
    assert_eq!(app.call(Method::GET, "/api/libraries", None, None, &[]).await.status, StatusCode::UNAUTHORIZED);
    let bad = app
        .call(Method::POST, "/api/auth/login", None, Some(json!({"username": "admin", "password": "nope"})), &[])
        .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);

    // Login sets an HttpOnly cookie that authenticates too.
    let login = app
        .call(Method::POST, "/api/auth/login", None, Some(json!({"username": "ADMIN", "password": "secret1"})), &[])
        .await;
    assert_eq!(login.status, StatusCode::OK);
    let cookie = login.headers[header::SET_COOKIE].to_str().unwrap().to_owned();
    assert!(cookie.contains("HttpOnly"));
    let cookie_pair = cookie.split(';').next().unwrap().to_owned();
    let me = app.call(Method::GET, "/api/auth/me", None, None, &[("cookie", &cookie_pair)]).await;
    assert_eq!(me.body["username"], json!("admin"));
    assert_eq!(me.body["is_admin"], json!(true));

    // A regular user cannot use admin endpoints.
    let created =
        app.post("/api/users", &admin, json!({"username": "bob", "password": "bobpass", "is_admin": false})).await;
    assert_eq!(created.status, StatusCode::CREATED);
    let bob = app
        .call(Method::POST, "/api/auth/login", None, Some(json!({"username": "bob", "password": "bobpass"})), &[])
        .await;
    let bob = bob.body["token"].as_str().unwrap().to_owned();
    assert_eq!(app.get("/api/users", &bob).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.get("/api/settings", &bob).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.get("/api/libraries", &bob).await.status, StatusCode::OK);
    assert_eq!(app.get("/api/activity", &bob).await.status, StatusCode::OK);

    // Wrong current password is a 400 (not a 401 that would sign the UI out).
    let change = app
        .call(
            Method::PUT,
            "/api/auth/password",
            Some(&bob),
            Some(json!({"current_password": "x", "new_password": "newpass"})),
            &[],
        )
        .await;
    assert_eq!(change.status, StatusCode::BAD_REQUEST);

    // Logout revokes the token.
    let out = app.call(Method::POST, "/api/auth/logout", Some(&bob), None, &[]).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/auth/me", &bob).await.status, StatusCode::UNAUTHORIZED);

    // Admins cannot delete themselves.
    let me_id = app.get("/api/auth/me", &admin).await.body["id"].as_i64().unwrap();
    let del = app.call(Method::DELETE, &format!("/api/users/{me_id}"), Some(&admin), None, &[]).await;
    assert_eq!(del.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn movies_are_scanned_listed_and_searchable() {
    let app = TestApp::new().await;
    let token = app.setup_admin().await;
    app.media_file("movies/The Matrix (1999)/The Matrix (1999).mkv", 2048);
    app.media_file("movies/The Matrix (1999)/The Matrix (1999).en.srt", 10);
    app.media_file("movies/The Matrix (1999)/poster.jpg", 10);
    app.media_file("movies/Blade.Runner.2049.2017.2160p.mkv", 2048);
    app.media_file("movies/Alien (1979).mp4", 4096);
    app.media_file("movies/sample.mkv", 10);
    let lib = app.library(&token, "Movies", "movies", "movies").await;

    let page = app.get(&format!("/api/libraries/{lib}/items"), &token).await;
    assert_eq!(page.status, StatusCode::OK, "{:?}", page.body);
    assert_eq!(page.body["total"], json!(3));
    assert_eq!(titles(&page.body["items"]), vec!["Alien", "Blade Runner 2049", "The Matrix"]);

    let by_year = app.get(&format!("/api/libraries/{lib}/items?sort=year&order=desc&limit=1"), &token).await;
    assert_eq!(by_year.body["total"], json!(3));
    assert_eq!(titles(&by_year.body["items"]), vec!["Blade Runner 2049"]);

    let matrix = page.body["items"].as_array().unwrap().iter().find(|i| i["title"] == "The Matrix").unwrap().clone();
    assert_eq!(matrix["year"], json!(1999));
    assert!(matrix["poster_url"].as_str().unwrap().contains("/images/poster"));
    let id = matrix["id"].as_i64().unwrap();
    let detail = app.get(&format!("/api/items/{id}"), &token).await;
    let streams = detail.body["files"][0]["streams"].as_array().unwrap();
    assert!(streams.iter().any(|s| s["external"] == json!(true) && s["language"] == json!("eng")));

    let poster = app.get(&format!("/api/items/{id}/images/poster?v=1"), &token).await;
    assert_eq!(poster.status, StatusCode::OK);
    assert!(poster.headers[header::CACHE_CONTROL].to_str().unwrap().contains("immutable"));

    let search = app.get("/api/search?q=matr", &token).await;
    assert_eq!(titles(&search.body["items"]), vec!["The Matrix"]);
    assert_eq!(app.get("/api/search?q=%20", &token).await.body["items"], json!([]));

    let library = app.get(&format!("/api/libraries/{lib}"), &token).await;
    assert_eq!(library.body["item_count"], json!(3));
    assert_eq!(library.body["scanning"], json!(false));

    // Removing a file on rescan removes the item; offline roots are not purged.
    std::fs::remove_file(app.media.path().join("movies/Alien (1979).mp4")).unwrap();
    let scan = app.post(&format!("/api/libraries/{lib}/scan"), &token, json!({"force": false})).await;
    assert_eq!(scan.status, StatusCode::ACCEPTED);
    app.wait_for_scans().await;
    assert_eq!(app.get(&format!("/api/libraries/{lib}/items"), &token).await.body["total"], json!(2));
}

#[tokio::test]
async fn shows_progress_and_hubs() {
    let app = TestApp::new().await;
    let token = app.setup_admin().await;
    for (s, e, title) in [(1, 1, "Pilot"), (1, 2, "Second"), (1, 3, "Third"), (2, 1, "Return")] {
        app.media_file(&format!("tv/My Show (2010)/Season {s:02}/My Show - S{s:02}E{e:02} - {title}.mkv"), 1024);
    }
    let lib = app.library(&token, "TV", "shows", "tv").await;

    let shows = app.get(&format!("/api/libraries/{lib}/items"), &token).await;
    assert_eq!(titles(&shows.body["items"]), vec!["My Show"]);
    let show = &shows.body["items"][0];
    assert_eq!(show["leaf_count"], json!(4));
    assert_eq!(show["child_count"], json!(2));
    assert_eq!(show["viewed_leaf_count"], json!(0));
    let show_id = show["id"].as_i64().unwrap();

    let seasons = app.get(&format!("/api/items/{show_id}/children"), &token).await;
    assert_eq!(titles(&seasons.body["items"]), vec!["Season 1", "Season 2"]);
    let season1 = seasons.body["items"][0]["id"].as_i64().unwrap();
    let episodes = app.get(&format!("/api/items/{season1}/children"), &token).await;
    assert_eq!(titles(&episodes.body["items"]), vec!["Pilot", "Second", "Third"]);
    let pilot = episodes.body["items"][0]["id"].as_i64().unwrap();
    let second = episodes.body["items"][1]["id"].as_i64().unwrap();
    assert_eq!(episodes.body["items"][0]["parent_index"], json!(1));
    assert_eq!(episodes.body["items"][0]["grandparent_title"], json!("My Show"));

    // On deck starts at the pilot; next follows the episode order across seasons.
    let deck = app.get(&format!("/api/items/{show_id}/on-deck"), &token).await;
    assert_eq!(deck.body["item"]["title"], json!("Pilot"));
    let third = episodes.body["items"][2]["id"].as_i64().unwrap();
    let next = app.get(&format!("/api/items/{third}/next"), &token).await;
    assert_eq!(next.body["item"]["title"], json!("Return"));

    // Finish the pilot, start the second episode.
    let r = app
        .post(
            &format!("/api/items/{pilot}/progress"),
            &token,
            json!({"position_ms": 2_900_000, "duration_ms": 3_000_000, "state": "stopped"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = app
        .post(
            &format!("/api/items/{second}/progress"),
            &token,
            json!({"position_ms": 600_000, "duration_ms": 3_000_000, "state": "paused"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);

    let pilot_detail = app.get(&format!("/api/items/{pilot}"), &token).await;
    assert_eq!(pilot_detail.body["user_state"]["watched"], json!(true));
    assert_eq!(pilot_detail.body["user_state"]["play_count"], json!(1));
    assert_eq!(pilot_detail.body["user_state"]["position_ms"], json!(0));

    let home = app.get("/api/hubs/home", &token).await;
    let hubs = home.body["hubs"].as_array().unwrap();
    let cw = hubs.iter().find(|h| h["key"] == "continue_watching").expect("continue watching hub");
    assert_eq!(titles(&cw["items"]), vec!["Second"]);
    assert_eq!(cw["items"][0]["user_state"]["position_ms"], json!(600_000));
    assert!(hubs.iter().any(|h| h["key"] == format!("recently_added:{lib}")));

    let deck = app.get(&format!("/api/items/{show_id}/on-deck"), &token).await;
    assert_eq!(deck.body["item"]["title"], json!("Second"));

    let lib_hubs = app.get(&format!("/api/libraries/{lib}/hubs"), &token).await;
    assert!(lib_hubs.body["hubs"].as_array().unwrap().iter().any(|h| h["key"] == format!("continue_watching:{lib}")));

    // Marking the show watched covers every episode; unwatching clears them.
    assert_eq!(
        app.post(&format!("/api/items/{show_id}/watched"), &token, json!({})).await.status,
        StatusCode::NO_CONTENT
    );
    let show = app.get(&format!("/api/items/{show_id}"), &token).await;
    assert_eq!(show.body["viewed_leaf_count"], json!(4));
    assert_eq!(show.body["user_state"]["watched"], json!(true));
    let unwatched = app.get(&format!("/api/libraries/{lib}/items?watched=false"), &token).await;
    assert_eq!(unwatched.body["total"], json!(0));
    assert_eq!(
        app.post(&format!("/api/items/{show_id}/unwatched"), &token, json!({})).await.status,
        StatusCode::NO_CONTENT
    );
    let show = app.get(&format!("/api/items/{show_id}"), &token).await;
    assert_eq!(show.body["viewed_leaf_count"], json!(0));
}

#[tokio::test]
async fn music_hierarchy_from_folders() {
    let app = TestApp::new().await;
    let token = app.setup_admin().await;
    app.media_file("music/Artist A/Album One (2001)/01 - Intro.mp3", 100);
    app.media_file("music/Artist A/Album One (2001)/02 - Song.mp3", 100);
    app.media_file("music/Artist B/Other/1-01 First.flac", 100);
    let lib = app.library(&token, "Music", "music", "music").await;

    let artists = app.get(&format!("/api/libraries/{lib}/items"), &token).await;
    assert_eq!(titles(&artists.body["items"]), vec!["Artist A", "Artist B"]);
    let artist = artists.body["items"][0]["id"].as_i64().unwrap();
    let albums = app.get(&format!("/api/items/{artist}/children"), &token).await;
    assert_eq!(titles(&albums.body["items"]), vec!["Album One"]);
    assert_eq!(albums.body["items"][0]["year"], json!(2001));
    let album = albums.body["items"][0]["id"].as_i64().unwrap();
    let tracks = app.get(&format!("/api/items/{album}/children"), &token).await;
    assert_eq!(titles(&tracks.body["items"]), vec!["Intro", "Song"]);
    assert_eq!(tracks.body["items"][1]["index"], json!(2));
    assert_eq!(tracks.body["items"][0]["grandparent_title"], json!("Artist A"));
}

#[tokio::test]
async fn direct_play_supports_ranges_and_transcode_needs_the_transcoder() {
    let app = TestApp::new().await;
    let token = app.setup_admin().await;
    app.media_file("movies/Clip (2020).mp4", 10_000);
    app.media_file("movies/Other (2020).mkv", 10_000);
    let lib = app.library(&token, "Movies", "movies", "movies").await;
    let items = app.get(&format!("/api/libraries/{lib}/items"), &token).await;
    let id_of = |title: &str| {
        items.body["items"].as_array().unwrap().iter().find(|i| i["title"] == title).unwrap()["id"].as_i64().unwrap()
    };
    let caps = json!({"containers": ["mp4", "webm"], "video_codecs": ["h264"], "audio_codecs": ["aac"]});

    let play = app.post("/api/playback", &token, json!({"item_id": id_of("Clip"), "caps": caps})).await;
    assert_eq!(play.status, StatusCode::OK, "{:?}", play.body);
    assert_eq!(play.body["method"], json!("direct"));
    let url = play.body["url"].as_str().unwrap().to_owned();
    let ranged = app.call(Method::GET, &url, Some(&token), None, &[("range", "bytes=100-199")]).await;
    assert_eq!(ranged.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(ranged.bytes.len(), 100);

    let activity = app.get("/api/activity", &token).await;
    assert_eq!(activity.body["sessions"].as_array().unwrap().len(), 1);
    let session = play.body["session_id"].as_str().unwrap();
    let stop = app.call(Method::DELETE, &format!("/api/playback/{session}"), Some(&token), None, &[]).await;
    assert_eq!(stop.status, StatusCode::NO_CONTENT);

    // MKV needs HLS from the transcoder, which is not running in tests.
    let play = app.post("/api/playback", &token, json!({"item_id": id_of("Other"), "caps": caps})).await;
    assert_eq!(play.status, StatusCode::SERVICE_UNAVAILABLE, "{:?}", play.body);

    // ...unless the player asks for the original file anyway.
    let body = json!({"item_id": id_of("Other"), "caps": caps, "mode": "direct", "client_id": "tab-1"});
    let play = app.post("/api/playback", &token, body).await;
    assert_eq!(play.status, StatusCode::OK, "{:?}", play.body);
    assert_eq!(play.body["method"], json!("direct"));

    // A client plays one thing at a time: its next session replaces the previous one.
    let first = play.body["session_id"].as_str().unwrap().to_owned();
    let body = json!({"item_id": id_of("Clip"), "caps": caps, "client_id": "tab-1"});
    let play = app.post("/api/playback", &token, body).await;
    assert_eq!(play.status, StatusCode::OK, "{:?}", play.body);
    let activity = app.get("/api/activity", &token).await;
    let sessions = activity.body["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1, "{sessions:?}");
    assert_ne!(play.body["session_id"].as_str().unwrap(), first);
    let other_client = json!({"item_id": id_of("Clip"), "caps": caps, "client_id": "tv"});
    app.post("/api/playback", &token, other_client).await;
    let activity = app.get("/api/activity", &token).await;
    assert_eq!(activity.body["sessions"].as_array().unwrap().len(), 2);

    let system = app.get("/api/system", &token).await;
    assert_eq!(system.body["transcoder"]["available"], json!(false));
    assert_eq!(system.body["counts"]["movies"], json!(2));
    let transcoder = app.get("/api/transcoder", &token).await;
    assert_eq!(transcoder.body["available"], json!(false));
}

#[tokio::test]
async fn settings_validation_and_folder_browser() {
    let app = TestApp::new().await;
    let token = app.setup_admin().await;
    let settings = app.get("/api/settings", &token).await;
    assert_eq!(settings.body["hwaccel"], json!("auto"));
    assert_eq!(settings.body["tonemapping"], json!("auto"));

    let updated = app
        .call(Method::PUT, "/api/settings", Some(&token), Some(json!({"max_transcodes": 2, "hwaccel": "nvenc"})), &[])
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.body["max_transcodes"], json!(2));
    assert_eq!(updated.body["hwaccel"], json!("nvenc"));
    let bad = app.call(Method::PUT, "/api/settings", Some(&token), Some(json!({"max_transcodes": 0})), &[]).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let bad = app.call(Method::PUT, "/api/settings", Some(&token), Some(json!({"hwaccel": "magic"})), &[]).await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);

    let roots = app.get("/api/fs", &token).await;
    assert!(!roots.body["entries"].as_array().unwrap().is_empty());
    std::fs::create_dir_all(app.media.path().join("Alpha")).unwrap();
    std::fs::create_dir_all(app.media.path().join("beta")).unwrap();
    std::fs::write(app.media.path().join("file.txt"), b"x").unwrap();
    let listing = app.get(&format!("/api/fs?path={}", urlencode(app.media.path())), &token).await;
    let names: Vec<_> =
        listing.body["entries"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_owned()).collect();
    assert_eq!(names, vec!["Alpha", "beta"]);

    let bad_lib =
        app.post("/api/libraries", &token, json!({"name": "X", "kind": "movies", "paths": ["relative/path"]})).await;
    assert_eq!(bad_lib.status, StatusCode::BAD_REQUEST);
}

fn urlencode(path: &Path) -> String {
    path.to_string_lossy()
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
