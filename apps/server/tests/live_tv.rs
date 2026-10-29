//! Live TV (docs/API.md, "Live TV"): a source's channels and guide read from
//! a stand-in IPTV provider, favourites, who may watch, and the connections
//! to the provider: one per channel however many watch it, no more than the
//! source allows, and a device changing channels freeing its own.

mod common;

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::StreamExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use common::TestApp;

/// A 1×1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00,
    0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[derive(Default)]
struct Provider {
    /// Streams open now, the most open at once, and opened in all.
    open: AtomicUsize,
    most: AtomicUsize,
    opened: AtomicUsize,
}

struct Open(Arc<Provider>);

impl Drop for Open {
    fn drop(&mut self) {
        self.0.open.fetch_sub(1, Ordering::SeqCst);
    }
}

fn guide_xml() -> Vec<u8> {
    let at = |offset: i64| {
        let t = chrono_like(pley::util::now() + offset);
        format!("{t} +0000")
    };
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<tv>
  <channel id="one.test"><display-name>One</display-name></channel>
  <channel id="two.test"><display-name>Two HD</display-name></channel>
  <programme start="{}" stop="{}" channel="one.test"><title>The News</title><desc>What happened.</desc><category>News</category></programme>
  <programme start="{}" stop="{}" channel="one.test"><title>The Film</title></programme>
  <programme start="{}" stop="{}" channel="two.test"><title>Football</title><live/></programme>
</tv>"#,
        at(-600),
        at(1200),
        at(1200),
        at(7200),
        at(-60),
        at(5400)
    );
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(xml.as_bytes()).unwrap();
    gz.finish().unwrap()
}

/// Unix seconds as XMLTV's `YYYYMMDDHHMMSS` (UTC).
fn chrono_like(t: i64) -> String {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    // Civil date from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}{:02}{:02}{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
}

/// The stand-in provider: a playlist naming its guide, the guide, logos,
/// and channels that send MPEG-TS packets for as long as they are read.
async fn provider() -> (String, Arc<Provider>) {
    let counts = Arc::new(Provider::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let playlist = format!(
        "#EXTM3U url-tvg=\"{base}/guide.xml.gz\"\n\
         #EXTINF:-1 tvg-id=\"one.test\" tvg-chno=\"1\" tvg-logo=\"{base}/logo.png\" group-title=\"News\",One\n\
         {base}/live/1.ts\n\
         #EXTINF:-1 tvg-chno=\"2\" group-title=\"Sport\",Two HD\n\
         {base}/live/2.ts\n\
         #EXTINF:-1 tvg-chno=\"10\" group-title=\"Sport\",Ten\n\
         {base}/live/10.ts\n\
         #EXTINF:-1 group-title=\"Films\",A film\n\
         {base}/movie/u/p/5.mkv\n"
    );
    let app = Router::new()
        .route("/list.m3u", get(move || async move { playlist }))
        .route("/guide.xml.gz", get(|| async { ([(header::CONTENT_TYPE, "application/gzip")], guide_xml()) }))
        .route("/logo.png", get(|| async { ([(header::CONTENT_TYPE, "image/png")], PNG) }))
        .route(
            "/live/{n}",
            get(|State(counts): State<Arc<Provider>>, Path(n): Path<String>| async move {
                let open = counts.open.fetch_add(1, Ordering::SeqCst) + 1;
                counts.most.fetch_max(open, Ordering::SeqCst);
                counts.opened.fetch_add(1, Ordering::SeqCst);
                let guard = Open(counts.clone());
                let id = n.trim_end_matches(".ts").parse::<u8>().unwrap_or(0);
                let packets = futures::stream::unfold(guard, move |guard| async move {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    let mut chunk = vec![0u8; 188 * 10];
                    for packet in chunk.chunks_mut(188) {
                        packet[0] = 0x47;
                        packet[4] = id;
                    }
                    Some((Ok::<_, std::io::Error>(bytes::Bytes::from(chunk)), guard))
                });
                ([(header::CONTENT_TYPE, "video/mp2t")], Body::from_stream(packets)).into_response()
            }),
        )
        .with_state(counts.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, counts)
}

async fn wait_until_read(app: &TestApp, admin: &str) -> Value {
    for _ in 0..100 {
        let sources = app.get("/api/live/sources", admin).await.body;
        let s = &sources[0];
        if s["guide_at"].is_i64() && !s["refreshing"].as_bool().unwrap_or(true) {
            return s.clone();
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the source wasn't read");
}

/// Starts reading a channel's MPEG-TS as `token`'s device `client`: the
/// status, and the body to read from.
async fn open_stream(app: &TestApp, token: &str, channel: i64, client: &str) -> (StatusCode, Body) {
    let req = Request::builder()
        .uri(format!("/api/live/channels/{channel}/stream.ts?client_id={client}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.api.clone().oneshot(req).await.unwrap();
    (resp.status(), resp.into_body())
}

/// The first bytes of a stream (a few chunks' worth).
async fn first_bytes(body: Body) -> (Vec<u8>, impl futures::Stream) {
    let mut stream = body.into_data_stream();
    let mut got = Vec::new();
    while got.len() < 188 * 20 {
        let chunk = tokio::time::timeout(Duration::from_secs(5), stream.next()).await.unwrap().unwrap().unwrap();
        got.extend_from_slice(&chunk);
    }
    (got, stream)
}

#[tokio::test]
async fn channels_guide_favourites_and_connections() {
    let (base, provider) = provider().await;
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (bob, _) = app.user(&admin, "bob", json!({})).await;

    // Nothing yet: the apps show no Live TV.
    let summary = app.get("/api/live", &bob).await.body;
    assert_eq!((summary["available"].as_bool(), summary["allowed"].as_bool()), (Some(false), Some(true)));

    let added = app
        .post(
            "/api/live/sources",
            &admin,
            json!({"name": "Provider", "kind": "m3u", "url": format!("{base}/list.m3u"), "max_connections": 1}),
        )
        .await;
    assert_eq!(added.status, StatusCode::CREATED, "{:?}", added.body);
    let source = wait_until_read(&app, &admin).await;
    assert_eq!(source["channel_count"], 3, "{source}");
    assert_eq!(source["guide_url"], format!("{base}/guide.xml.gz"), "the playlist's own guide is taken");
    assert_eq!(source["guided_count"], 2, "Two HD is matched to the guide's Two by its name");
    assert!(source["channels_error"].is_null() && source["guide_error"].is_null(), "{source}");

    let channels = app.get("/api/live/channels", &bob).await.body;
    let names: Vec<&str> = channels.as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["One", "Two HD", "Ten"], "films are left out, channels go by number");
    let one = &channels[0];
    let (one_id, two_id) = (one["id"].as_i64().unwrap(), channels[1]["id"].as_i64().unwrap());
    assert_eq!(one["now"]["title"], "The News");
    assert_eq!(one["now"]["description"], "What happened.");
    assert_eq!(one["next"]["title"], "The Film");
    assert_eq!(channels[1]["now"]["title"], "Football");
    assert_eq!(channels[1]["now"]["is_live"], true);
    assert!(channels[2]["now"].is_null() && channels[2]["has_guide"] == false);

    // The logo comes through the server.
    let logo = app.get(one["logo"].as_str().unwrap(), &bob).await;
    assert_eq!(logo.status, StatusCode::OK);
    assert_eq!(logo.bytes, PNG);
    assert!(channels[1]["logo"].is_null());

    // Favourites are each user's own.
    assert_eq!(
        app.put(&format!("/api/live/channels/{two_id}/favorite"), &bob, json!({})).await.status,
        StatusCode::NO_CONTENT
    );
    let bobs = app.get("/api/live/channels", &bob).await.body;
    assert_eq!(bobs[1]["favorite"], true);
    assert_eq!(app.get("/api/live/channels", &admin).await.body[1]["favorite"], false);

    let guide = app.get("/api/live/guide", &bob).await.body;
    assert_eq!(guide["programs"][one_id.to_string()].as_array().unwrap().len(), 2, "{guide}");

    // Two watch One: one connection to the provider.
    let (status, body) = open_stream(&app, &admin, one_id, "tv").await;
    assert_eq!(status, StatusCode::OK);
    let (first, admin_stream) = first_bytes(body).await;
    assert_eq!((first[0], first[4]), (0x47, 1), "the provider's packets come as they are");
    let (status, body) = open_stream(&app, &bob, one_id, "phone").await;
    assert_eq!(status, StatusCode::OK);
    let (_, bob_stream) = first_bytes(body).await;
    assert_eq!(provider.opened.load(Ordering::SeqCst), 1, "one connection for both");

    // The one connection is in use: Bob can't tune Two as well.
    let refused = app.post(&format!("/api/live/channels/{two_id}/watch"), &bob, json!({"client_id": "laptop"})).await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{:?}", refused.body);
    assert!(refused.body["message"].as_str().unwrap().contains("watching One"), "{:?}", refused.body);

    let tuners = app.get("/api/live/tuners", &admin).await.body;
    assert_eq!(tuners.as_array().unwrap().len(), 1);
    assert_eq!(tuners[0]["viewers"].as_array().unwrap().len(), 2, "{tuners}");

    // Bob stops; the TV changes to Two: its connection goes to Two.
    drop(bob_stream);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let (status, body) = open_stream(&app, &admin, two_id, "tv").await;
    assert_eq!(status, StatusCode::OK);
    drop(admin_stream);
    let (first, _two) = first_bytes(body).await;
    assert_eq!(first[4], 2, "Two's packets");
    for _ in 0..50 {
        if provider.open.load(Ordering::SeqCst) == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(provider.open.load(Ordering::SeqCst), 1);
    assert_eq!(provider.most.load(Ordering::SeqCst), 1, "never more than the one connection at once");
    assert_eq!(provider.opened.load(Ordering::SeqCst), 2);

    let summary = app.get("/api/live", &bob).await.body;
    assert_eq!(summary["available"], true);
    assert_eq!(summary["recent"], json!([one_id]));

    // Hidden channels are nobody's but the administrators'.
    let hidden = app.patch("/api/live/lineup", &admin, json!({"ids": [two_id], "hidden": true})).await;
    assert_eq!(hidden.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/live/channels", &bob).await.body.as_array().unwrap().len(), 2);
    assert_eq!(app.get(&format!("/api/live/channels/{two_id}"), &bob).await.status, StatusCode::NOT_FOUND);
    let lineup = app.get("/api/live/lineup", &admin).await.body;
    assert_eq!(lineup.as_array().unwrap().len(), 3);
    assert!(app.get("/api/live/tuners", &admin).await.body.as_array().unwrap().is_empty(), "hiding it stopped it");

    // Renamed and renumbered, kept across a refresh.
    let renamed =
        app.patch("/api/live/lineup", &admin, json!({"ids": [one_id], "name": "One (UK)", "number": "50"})).await;
    assert_eq!(renamed.status, StatusCode::NO_CONTENT);
    let id = source["id"].as_i64().unwrap();
    assert_eq!(
        app.post(&format!("/api/live/sources/{id}/refresh"), &admin, json!({})).await.status,
        StatusCode::ACCEPTED
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    wait_until_read(&app, &admin).await;
    let names: Vec<String> = app
        .get("/api/live/channels", &bob)
        .await
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["Ten", "One (UK)"]);

    // Administrators only: Bob sees no Live TV.
    let settings = app.get("/api/live/settings", &admin).await.body;
    let mut admins_only = settings.clone();
    admins_only["access"] = json!("admins");
    assert_eq!(app.put("/api/live/settings", &admin, admins_only).await.status, StatusCode::OK);
    assert_eq!(app.get("/api/live/channels", &bob).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.get("/api/live", &bob).await.body["allowed"], false);
    assert_eq!(app.get("/api/live/sources", &bob).await.status, StatusCode::FORBIDDEN);
    let mut bad = settings;
    bad["timeshift_minutes"] = json!(0);
    assert_eq!(app.put("/api/live/settings", &admin, bad).await.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sources_are_tested_and_their_errors_said() {
    let (base, _) = provider().await;
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let test = app
        .post("/api/live/sources/test", &admin, json!({"name": "P", "kind": "m3u", "url": format!("{base}/list.m3u")}))
        .await;
    assert_eq!(test.status, StatusCode::OK, "{:?}", test.body);
    assert_eq!((test.body["channels"].as_i64(), test.body["groups"].as_i64()), (Some(3), Some(2)));
    assert_eq!(test.body["guide_url"], format!("{base}/guide.xml.gz"));
    let missing = app
        .post(
            "/api/live/sources/test",
            &admin,
            json!({"name": "P", "kind": "m3u", "url": format!("{base}/nothing.m3u")}),
        )
        .await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST);
    assert!(missing.body["message"].as_str().unwrap().contains("404"), "{:?}", missing.body);
    let bad = app.post("/api/live/sources", &admin, json!({"name": "P", "kind": "m3u", "url": "ftp://x"})).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let xtream = app
        .post(
            "/api/live/sources",
            &admin,
            json!({"name": "X", "kind": "xtream", "url": format!("{base}/"), "username": "", "password": "p"}),
        )
        .await;
    assert_eq!(xtream.status, StatusCode::BAD_REQUEST, "an Xtream account needs its user name");
    let added = app
        .post(
            "/api/live/sources",
            &admin,
            json!({"name": "Broken", "kind": "m3u", "url": format!("{base}/nothing.m3u")}),
        )
        .await;
    assert_eq!(added.status, StatusCode::CREATED);
    let mut error = Value::Null;
    for _ in 0..50 {
        error = app.get("/api/live/sources", &admin).await.body[0]["channels_error"].clone();
        if error.is_string() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(error.as_str().is_some_and(|e| e.contains("404")), "{error}");
    let id = added.body["id"].as_i64().unwrap();
    assert_eq!(app.delete(&format!("/api/live/sources/{id}"), &admin).await.status, StatusCode::NO_CONTENT);
    assert!(app.get("/api/live/sources", &admin).await.body.as_array().unwrap().is_empty());
}
