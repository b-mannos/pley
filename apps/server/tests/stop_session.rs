//! Stopping a playback session from elsewhere (docs/API.md, "Stopping a
//! stream"): `DELETE /api/sessions/{id}` and the Plex API's
//! `/status/sessions/terminate`.

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use futures::StreamExt;
use serde_json::{Value, json};

use common::{TestApp, encode};

/// What a browser playing Matroska directly says it plays.
const CAPS: &str = r#"{"containers": ["mkv"], "video_codecs": ["h264"], "audio_codecs": ["aac"]}"#;

struct World {
    app: TestApp,
    admin: String,
    viewer: String,
    viewer_id: i64,
    other: String,
    film: i64,
}

async fn world() -> World {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.media_file("movies/Film (2001)/Film (2001).mkv", 2048);
    let movies = app.library(&admin, "Movies", "movies", "movies").await;
    let film = app.ids(movies, &admin).await["Film"];
    let (viewer, viewer_id) = app.user(&admin, "viewer", json!({})).await;
    let (other, _) = app.user(&admin, "other", json!({})).await;
    World { app, admin, viewer, viewer_id, other, film }
}

/// A direct play of the film by `token`'s user: its session id.
async fn play(w: &World, token: &str) -> String {
    let caps: Value = serde_json::from_str(CAPS).unwrap();
    let started = w.app.post("/api/playback", token, json!({"item_id": w.film, "caps": caps})).await;
    assert_eq!(started.status, StatusCode::OK, "{}", started.text());
    assert_eq!(started.body["method"], json!("direct"));
    started.body["session_id"].as_str().unwrap().to_owned()
}

/// The events stream of `token`'s user, read until `until` shows up (or 10 s).
async fn events_until(w: &World, token: &str, act: impl Future<Output = ()>, until: &str) -> String {
    let request = axum::http::Request::builder()
        .uri("/api/events")
        .header("authorization", format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(w.app.api.clone(), request).await.unwrap();
    let mut stream = response.into_body().into_data_stream();
    act.await;
    let mut received = String::new();
    let read = async {
        while let Some(chunk) = stream.next().await {
            received.push_str(&String::from_utf8_lossy(&chunk.unwrap()));
            if received.contains(until) {
                return;
            }
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(10), read).await;
    received
}

/// The `session_stopped` event among what an events stream sent.
fn stopped_event(received: &str) -> Value {
    received
        .lines()
        .filter_map(|line| line.strip_prefix("data: ").or_else(|| line.strip_prefix("data:")))
        .filter_map(|data| serde_json::from_str::<Value>(data).ok())
        .find(|event| event["type"] == "session_stopped")
        .unwrap_or_else(|| panic!("no session_stopped event in {received}"))
}

#[tokio::test]
async fn administrators_stop_any_session_and_its_player_hears_it() {
    let w = world().await;
    let app = &w.app;
    let session = play(&w, &w.viewer).await;
    let uri = format!("/api/sessions/{session}");

    // Someone else's session: not theirs to stop.
    let refused = app.delete(&uri, &w.other).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.text());
    assert_eq!(app.delete("/api/sessions/no-such-session", &w.admin).await.status, StatusCode::NOT_FOUND);
    let long = json!({"message": "x".repeat(201)});
    let too_long = app.call(Method::DELETE, &uri, Some(w.admin.as_str()), Some(long), &[]).await;
    assert_eq!(too_long.status, StatusCode::BAD_REQUEST);

    // The administrator stops it with a message: the player's stream hears
    // it, and everyone's hears that sessions changed.
    let mut events = app.state.events.subscribe();
    let message = json!({"message": "  Server maintenance in 5 minutes  "});
    let stop = async {
        let stopped = app.call(Method::DELETE, &uri, Some(w.admin.as_str()), Some(message), &[]).await;
        assert_eq!(stopped.status, StatusCode::NO_CONTENT, "{}", stopped.text());
    };
    let heard = events_until(&w, &w.viewer, stop, "session_stopped").await;
    let expected = json!({
        "type": "session_stopped", "session_id": session, "message": "Server maintenance in 5 minutes", "by_admin": true,
    });
    assert_eq!(stopped_event(&heard), expected);
    let mut kinds = Vec::new();
    while let Ok(event) = events.try_recv() {
        kinds.push(serde_json::to_value(&event).unwrap()["type"].as_str().unwrap().to_owned());
    }
    assert!(kinds.contains(&"sessions_changed".to_owned()), "{kinds:?}");
    assert!(app.get("/api/activity", &w.admin).await.body["sessions"].as_array().unwrap().is_empty());

    // The player's next requests answer 410 with the message.
    let gone = app.get(&format!("/api/playback/{session}"), &w.viewer).await;
    assert_eq!(gone.status, StatusCode::GONE);
    assert_eq!(
        gone.body,
        json!({"code": "session_stopped", "error": "session_stopped", "message": "Server maintenance in 5 minutes"})
    );
    let report = json!({"position_ms": 20_000, "duration_ms": 100_000, "session_id": session});
    let reported = app.post(&format!("/api/items/{}/progress", w.film), &w.viewer, report).await;
    assert_eq!(reported.status, StatusCode::GONE);
    // The place is kept all the same, and the session isn't started again.
    let state = &app.get(&format!("/api/items/{}", w.film), &w.viewer).await.body["user_state"];
    assert_eq!(state["position_ms"], json!(20_000));
    assert!(app.get("/api/activity", &w.admin).await.body["sessions"].as_array().unwrap().is_empty());
    assert_eq!(app.get(&format!("/api/playback/{session}/index.m3u8"), &w.viewer).await.status, StatusCode::GONE);
    // Other users still don't learn about it.
    assert_eq!(app.get(&format!("/api/playback/{session}"), &w.other).await.status, StatusCode::NOT_FOUND);
    // The player closing it, or stopping it again, is fine.
    assert_eq!(app.delete(&format!("/api/playback/{session}"), &w.viewer).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.delete(&uri, &w.admin).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.delete(&uri, &w.other).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn users_stop_their_own_sessions() {
    let w = world().await;
    let app = &w.app;
    let session = play(&w, &w.viewer).await;
    // From another device: an app with the play scope, without a message.
    let (phone, _) = app.app_token(w.viewer_id, "library play").await;
    let (browse, _) = app.app_token(w.viewer_id, "library").await;
    let uri = format!("/api/sessions/{session}");
    assert_eq!(app.app_call(Method::DELETE, &uri, &browse, None).await.status, StatusCode::FORBIDDEN);
    let stop = async {
        assert_eq!(app.app_call(Method::DELETE, &uri, &phone, None).await.status, StatusCode::NO_CONTENT);
    };
    let heard = events_until(&w, &w.viewer, stop, "session_stopped").await;
    let expected = json!({"type": "session_stopped", "session_id": session, "message": null, "by_admin": false});
    assert_eq!(stopped_event(&heard), expected);
    let gone = app.get(&format!("/api/playback/{session}"), &w.viewer).await;
    assert_eq!((gone.status, gone.body["message"].clone()), (StatusCode::GONE, Value::Null));

    // An administrator's own session is theirs like anyone's.
    let own = play(&w, &w.admin).await;
    let heard = events_until(
        &w,
        &w.admin,
        async {
            app.delete(&format!("/api/sessions/{own}"), &w.admin).await;
        },
        "session_stopped",
    )
    .await;
    assert_eq!(stopped_event(&heard)["by_admin"], json!(false));
}

#[tokio::test]
async fn plex_tools_terminate_sessions() {
    let w = world().await;
    let app = &w.app;
    let session = play(&w, &w.viewer).await;
    let sessions = app.plex_json("/status/sessions", &w.admin).await;
    assert_eq!(sessions["Metadata"][0]["Session"]["id"], json!(session));

    let terminate = format!("/status/sessions/terminate?sessionId={session}&reason={}", encode("Too many streams"));
    assert_eq!(app.plex(Method::GET, &terminate, &w.viewer, &[]).await.status, StatusCode::FORBIDDEN);
    let missing = app.plex(Method::GET, "/status/sessions/terminate", &w.admin, &[]).await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST);
    let unknown = app.plex(Method::GET, "/status/sessions/terminate?sessionId=nope", &w.admin, &[]).await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    let ended = app.plex(Method::GET, &terminate, &w.admin, &[]).await;
    assert_eq!(ended.status, StatusCode::OK, "{}", ended.text());
    assert_eq!(app.plex_json("/status/sessions", &w.admin).await["size"], json!(0));
    let gone = app.get(&format!("/api/playback/{session}"), &w.viewer).await;
    assert_eq!((gone.status, gone.body["message"].clone()), (StatusCode::GONE, json!("Too many streams")));

    // A Plex player's reports don't start a stopped session again.
    let client = [("x-plex-client-identifier", "living-room")];
    let timeline = format!("/:/timeline?ratingKey={}&time=10000&state=playing", w.film);
    app.plex(Method::GET, &timeline, &w.viewer, &client).await;
    let plex_session = app.plex_json("/status/sessions", &w.admin).await["Metadata"][0]["Session"]["id"].clone();
    let plex_session = plex_session.as_str().unwrap().to_owned();
    let terminate = format!("/status/sessions/terminate?sessionId={plex_session}");
    assert_eq!(app.plex(Method::GET, &terminate, &w.admin, &[]).await.status, StatusCode::OK);
    app.plex(Method::GET, &timeline, &w.viewer, &client).await;
    assert_eq!(app.plex_json("/status/sessions", &w.admin).await["size"], json!(0));

    // Once that player stops too, playing the title again shows up.
    let stop = format!("/:/timeline?ratingKey={}&time=12000&state=stopped", w.film);
    app.plex(Method::GET, &stop, &w.viewer, &client).await;
    app.plex(Method::GET, &timeline, &w.viewer, &client).await;
    assert_eq!(app.plex_json("/status/sessions", &w.admin).await["size"], json!(1));
}
