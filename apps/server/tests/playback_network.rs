//! What Automatic quality knows of a client's network (`GET` and `PUT
//! /api/playback/network`, src/bandwidth.rs) and the speed test that
//! measures it (`GET /api/playback/speedtest`), as clients reach them: from
//! the internet, from home, through a proxy on the server's network.

mod common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{Reply, TestApp};

const NETWORK: &str = "/api/playback/network";
/// A home on the internet, and another.
const HOME: &str = "81.2.69.160:51000";
const CAFE: &str = "203.0.113.7:40000";

/// A request with `token` from `peer` (`None`: no connection, as in the
/// other tests), with `extra` headers.
async fn send(
    app: &TestApp,
    method: Method,
    uri: &str,
    token: &str,
    body: Option<Value>,
    peer: Option<&str>,
    extra: &[(&str, &str)],
) -> axum::response::Response {
    let mut req = Request::builder().method(method).uri(uri).header(header::AUTHORIZATION, format!("Bearer {token}"));
    for (name, value) in extra {
        req = req.header(*name, *value);
    }
    if let Some(peer) = peer {
        req = req.extension(axum::extract::ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
    }
    let req = match body {
        Some(b) => req.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())).unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    app.api.clone().oneshot(req).await.unwrap()
}

async fn reply(resp: axum::response::Response) -> Reply {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), 128 * 1024 * 1024).await.unwrap().to_vec();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Reply { status, headers, body, bytes }
}

async fn get(app: &TestApp, token: &str, client: &str, peer: Option<&str>) -> Value {
    let r = reply(send(app, Method::GET, &format!("{NETWORK}?client_id={client}"), token, None, peer, &[]).await).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.body
}

async fn put(app: &TestApp, token: &str, body: Value, peer: Option<&str>) -> Reply {
    reply(send(app, Method::PUT, NETWORK, token, Some(body), peer, &[]).await).await
}

async fn put_ok(app: &TestApp, token: &str, body: Value, peer: Option<&str>) -> Value {
    let r = put(app, token, body, peer).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.body
}

fn nothing_known(probe: bool) -> Value {
    json!({"local": false, "ceiling_kbps": null, "source": null, "since": null, "probe": probe})
}

#[tokio::test]
async fn a_client_away_from_home_measures_once_then_starts_at_what_it_found() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (ann, _) = app.user(&admin, "ann", json!({})).await;

    assert_eq!(get(&app, &ann, "phone", Some(HOME)).await, nothing_known(true));

    let before = pley::util::now_ms();
    let found =
        put_ok(&app, &ann, json!({"client_id": "phone", "probe_kbps": 52000, "probe_complete": false}), Some(HOME))
            .await;
    assert_eq!(found["ceiling_kbps"], 36400, "70 % of the test: {found}");
    assert_eq!(found["source"], "probe");
    assert_eq!(found["probe"], false);
    assert_eq!(found["local"], false);
    assert!(found["since"].as_i64().unwrap() >= before, "{found}");
    assert_eq!(get(&app, &ann, "phone", Some(HOME)).await, found, "kept");

    // Per client, per network, per user.
    assert_eq!(get(&app, &ann, "tv", Some(HOME)).await, nothing_known(true));
    assert_eq!(get(&app, &ann, "phone", Some(CAFE)).await, nothing_known(true));
    let (bob, _) = app.user(&admin, "bob", json!({})).await;
    assert_eq!(get(&app, &bob, "phone", Some(HOME)).await, nothing_known(true));

    // A step down wins over the test; a step up to a quality moves it.
    let down = put_ok(&app, &ann, json!({"client_id": "phone", "ceiling_kbps": 8000}), Some(HOME)).await;
    assert_eq!((down["ceiling_kbps"].clone(), down["source"].clone()), (json!(8000), json!("playback")));
    let up = put_ok(&app, &ann, json!({"client_id": "phone", "ceiling_kbps": 20000.4}), Some(HOME)).await;
    assert_eq!(up["ceiling_kbps"], 20000, "whole kbps");

    // Back to Original: no limit, and no test due.
    let forgot = put_ok(&app, &ann, json!({"client_id": "phone", "ceiling_kbps": null}), Some(HOME)).await;
    assert_eq!(forgot, nothing_known(false));
    assert_eq!(get(&app, &ann, "phone", Some(HOME)).await, nothing_known(false));

    // A test that arrived whole limits nothing either.
    let fast =
        put_ok(&app, &ann, json!({"client_id": "tv", "probe_kbps": 140000, "probe_complete": true}), Some(HOME)).await;
    assert_eq!(fast, nothing_known(false));
}

#[tokio::test]
async fn devices_of_one_home_share_their_ipv6_network() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    put_ok(&app, &admin, json!({"client_id": "tv", "probe_kbps": 10000}), Some("[2001:db8:1:2::10]:443")).await;
    let same = get(&app, &admin, "tv", Some("[2001:db8:1:2:aaaa:bbbb:cccc:dddd]:443")).await;
    assert_eq!(same["ceiling_kbps"], 7000, "{same}");
    assert_eq!(get(&app, &admin, "tv", Some("[2001:db8:1:3::10]:443")).await, nothing_known(true));
}

#[tokio::test]
async fn at_home_nothing_is_measured_but_ceilings_are_kept() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let home = json!({"local": true, "ceiling_kbps": null, "source": null, "since": null, "probe": false});
    for peer in [Some("192.168.1.20:5000"), Some("127.0.0.1:5000"), Some("[fd00::5]:5000"), None] {
        assert_eq!(get(&app, &admin, "tv", peer).await, home, "{peer:?}");
    }
    let down = put_ok(&app, &admin, json!({"client_id": "tv", "ceiling_kbps": 8000}), Some("192.168.1.20:5000")).await;
    assert_eq!(down["ceiling_kbps"], 8000);
    assert_eq!(down["local"], true);
    // Every address at home is one network.
    assert_eq!(get(&app, &admin, "tv", Some("10.0.0.3:5000")).await["ceiling_kbps"], 8000);

    // Through a proxy on the server's network: the client it forwards for.
    let proxied = reply(
        send(
            &app,
            Method::GET,
            &format!("{NETWORK}?client_id=tv"),
            &admin,
            None,
            Some("127.0.0.1:5000"),
            &[("x-forwarded-for", "81.2.69.160")],
        )
        .await,
    )
    .await;
    assert_eq!(proxied.body, nothing_known(true));
    // Which nobody on the internet can claim to be.
    let claimed = reply(
        send(
            &app,
            Method::GET,
            &format!("{NETWORK}?client_id=tv"),
            &admin,
            None,
            Some(CAFE),
            &[("x-forwarded-for", "192.168.1.20")],
        )
        .await,
    )
    .await;
    assert_eq!(claimed.body["local"], false);
}

#[tokio::test]
async fn the_address_is_never_stored_and_old_rows_go() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    put_ok(&app, &admin, json!({"client_id": "tv", "probe_kbps": 10000}), Some(HOME)).await;
    let networks: Vec<String> =
        sqlx::query_scalar("SELECT network FROM playback_networks").fetch_all(&app.state.db.read).await.unwrap();
    assert_eq!(networks.len(), 1);
    assert_eq!(networks[0].len(), 32, "{networks:?}");
    assert!(networks[0].chars().all(|c| c.is_ascii_hexdigit()) && !networks[0].contains("81.2"), "{networks:?}");

    // A day later the step down no longer counts; a week later the test neither.
    put_ok(&app, &admin, json!({"client_id": "tv", "ceiling_kbps": 4000}), Some(HOME)).await;
    let day = 24 * 3600 * 1000;
    sqlx::query("UPDATE playback_networks SET ceiling_at = ceiling_at - ?")
        .bind(day)
        .execute(&app.state.db.write)
        .await
        .unwrap();
    let after_a_day = get(&app, &admin, "tv", Some(HOME)).await;
    assert_eq!((after_a_day["ceiling_kbps"].clone(), after_a_day["source"].clone()), (json!(7000), json!("probe")));
    sqlx::query("UPDATE playback_networks SET probe_at = probe_at - ?")
        .bind(7 * day)
        .execute(&app.state.db.write)
        .await
        .unwrap();
    assert_eq!(get(&app, &admin, "tv", Some(HOME)).await, nothing_known(true));

    // Rows nobody wrote for a week go with the next report.
    sqlx::query("UPDATE playback_networks SET updated_at = updated_at - ?")
        .bind(7 * day)
        .execute(&app.state.db.write)
        .await
        .unwrap();
    put_ok(&app, &admin, json!({"client_id": "phone", "ceiling_kbps": 4000}), Some(CAFE)).await;
    let clients: Vec<String> =
        sqlx::query_scalar("SELECT client_id FROM playback_networks").fetch_all(&app.state.db.read).await.unwrap();
    assert_eq!(clients, ["phone"]);
}

#[tokio::test]
async fn wrong_requests_are_refused() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    for uri in [NETWORK.to_owned(), format!("{NETWORK}?client_id="), format!("{NETWORK}?client_id={}", "x".repeat(129))]
    {
        let r = reply(send(&app, Method::GET, &uri, &admin, None, Some(HOME), &[]).await).await;
        assert_eq!((r.status, r.body["code"].clone()), (StatusCode::BAD_REQUEST, json!("bad_request")), "{uri}");
    }
    let wrong = [
        json!({"probe_kbps": 5000}),
        json!({"client_id": "", "probe_kbps": 5000}),
        json!({"client_id": "x".repeat(129), "probe_kbps": 5000}),
        json!({"client_id": 7, "probe_kbps": 5000}),
        json!({"client_id": "tv"}),
        json!({"client_id": "tv", "probe_kbps": 5000, "ceiling_kbps": 5000}),
        json!({"client_id": "tv", "probe_kbps": 5000, "ceiling_kbps": null}),
        json!({"client_id": "tv", "probe_kbps": 0}),
        json!({"client_id": "tv", "probe_kbps": -1}),
        json!({"client_id": "tv", "probe_kbps": 10_000_001}),
        json!({"client_id": "tv", "ceiling_kbps": 0}),
        json!({"client_id": "tv", "ceiling_kbps": "8000"}),
        json!({"client_id": "tv", "probe_kbps": 5000, "probe_complete": "yes"}),
        json!("tv"),
    ];
    for body in wrong {
        let r = put(&app, &admin, body.clone(), Some(HOME)).await;
        assert_eq!((r.status, r.body["code"].clone()), (StatusCode::BAD_REQUEST, json!("bad_request")), "{body}");
    }
    let not_json = reply(
        app.api
            .clone()
            .oneshot(
                Request::put(NETWORK)
                    .header(header::AUTHORIZATION, format!("Bearer {admin}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{"))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(not_json.status, StatusCode::BAD_REQUEST);
    // Nothing kept from any of them; the largest speed is fine.
    assert_eq!(get(&app, &admin, "tv", Some(HOME)).await, nothing_known(true));
    put_ok(&app, &admin, json!({"client_id": "x".repeat(128), "probe_kbps": 10_000_000}), Some(HOME)).await;

    // Signed in only.
    let anyone =
        reply(send(&app, Method::GET, &format!("{NETWORK}?client_id=tv"), "nobody", None, Some(HOME), &[]).await).await;
    assert_eq!(anyone.status, StatusCode::UNAUTHORIZED);
    let test = reply(send(&app, Method::GET, "/api/playback/speedtest", "nobody", None, Some(HOME), &[]).await).await;
    assert_eq!(test.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_speed_test_sends_exactly_what_was_asked_uncompressed() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let gzip = [("accept-encoding", "gzip, br, zstd")];
    let r =
        reply(send(&app, Method::GET, "/api/playback/speedtest?bytes=100000", &admin, None, None, &gzip).await).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.bytes.len(), 100_000);
    assert_eq!(r.headers[header::CONTENT_LENGTH], "100000");
    assert_eq!(r.headers[header::CONTENT_TYPE], "application/octet-stream");
    assert!(r.headers[header::CACHE_CONTROL].to_str().unwrap().starts_with("no-store"), "{:?}", r.headers);
    assert!(!r.headers.contains_key(header::CONTENT_ENCODING), "{:?}", r.headers);
    // Nothing repeats: each 64 KiB differs, and so does the next test.
    assert_ne!(r.bytes[..32_768], r.bytes[32_768..65_536]);
    let again =
        reply(send(&app, Method::GET, "/api/playback/speedtest?bytes=100000", &admin, None, None, &[]).await).await;
    assert_ne!(again.bytes, r.bytes);

    let whole = reply(send(&app, Method::GET, "/api/playback/speedtest", &admin, None, None, &gzip).await).await;
    assert_eq!(whole.headers[header::CONTENT_LENGTH], "33554432");
    assert_eq!(whole.bytes.len(), 33_554_432, "32 MiB unless asked");
    for (asked, size) in [("1", 65_536), ("100000000", 67_108_864)] {
        let resp =
            send(&app, Method::GET, &format!("/api/playback/speedtest?bytes={asked}"), &admin, None, None, &[]).await;
        assert_eq!(resp.headers()[header::CONTENT_LENGTH], size.to_string().as_str(), "{asked}");
    }
    for wrong in ["abc", "-5", "1.5"] {
        let r = reply(
            send(&app, Method::GET, &format!("/api/playback/speedtest?bytes={wrong}"), &admin, None, None, &[]).await,
        )
        .await;
        assert_eq!((r.status, r.body["code"].clone()), (StatusCode::BAD_REQUEST, json!("bad_request")), "{wrong}");
    }
}

async fn speed_test(app: &TestApp, token: &str) -> axum::response::Response {
    send(app, Method::GET, "/api/playback/speedtest", token, None, None, &[]).await
}

#[tokio::test]
async fn two_speed_tests_at_a_time_per_user() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (ann, _) = app.user(&admin, "ann", json!({})).await;
    // Answered, not yet read.
    let first = speed_test(&app, &admin).await;
    let second = speed_test(&app, &admin).await;
    assert_eq!((first.status(), second.status()), (StatusCode::OK, StatusCode::OK));
    let third = reply(speed_test(&app, &admin).await).await;
    assert_eq!((third.status, third.body["code"].clone()), (StatusCode::TOO_MANY_REQUESTS, json!("too_many_requests")));
    assert_eq!(speed_test(&app, &ann).await.status(), StatusCode::OK, "another user's own two");
    // A test read to its end, or dropped (the player stopped it), gives its turn back.
    reply(first).await;
    let fourth = speed_test(&app, &admin).await;
    assert_eq!(fourth.status(), StatusCode::OK);
    drop(second);
    assert_eq!(speed_test(&app, &admin).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn apps_allowed_to_play_use_them() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (_, ann) = app.user(&admin, "ann", json!({})).await;
    let (player, _) = app.app_token(ann, "library play").await;
    let (browser, _) = app.app_token(ann, "library").await;
    assert_eq!(get(&app, &player, "tv", Some(HOME)).await, nothing_known(true));
    put_ok(&app, &player, json!({"client_id": "tv", "ceiling_kbps": 8000}), Some(HOME)).await;
    let r = reply(send(&app, Method::GET, "/api/playback/speedtest?bytes=65536", &player, None, None, &[]).await).await;
    assert_eq!((r.status, r.bytes.len()), (StatusCode::OK, 65_536));

    let uri = format!("{NETWORK}?client_id=tv");
    for (method, uri, body) in [
        (Method::GET, uri.as_str(), None),
        (Method::PUT, NETWORK, Some(json!({"client_id": "tv", "ceiling_kbps": 8000}))),
        (Method::GET, "/api/playback/speedtest", None),
    ] {
        let r = reply(send(&app, method.clone(), uri, &browser, body, Some(HOME), &[]).await).await;
        assert_eq!(
            (r.status, r.body["code"].clone()),
            (StatusCode::FORBIDDEN, json!("insufficient_scope")),
            "{method} {uri}"
        );
    }
}
