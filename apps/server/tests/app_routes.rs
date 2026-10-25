//! What pley's own apps do beyond playing and browsing: set the profile's
//! track defaults (`PUT /api/me/track-defaults`, src/api/accounts.rs).

mod common;

use axum::http::{StatusCode, header};
use serde_json::{Value, json};

use common::TestApp;

// ------------------------------------------------------------------ track defaults

const DEFAULTS: &str = "/api/me/track-defaults";

/// A profile's track defaults as stored: `(audio, subtitles, mode)`.
async fn stored(app: &TestApp, id: i64) -> (Option<String>, Option<String>, String) {
    sqlx::query_as("SELECT audio_language, subtitle_language, subtitle_mode FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&app.state.db.read)
        .await
        .unwrap()
}

fn defaults(audio: Option<&str>, subtitles: Option<&str>, mode: &str) -> (Option<String>, Option<String>, String) {
    (audio.map(str::to_owned), subtitles.map(str::to_owned), mode.to_owned())
}

#[tokio::test]
async fn an_app_allowed_play_sets_the_profiles_track_defaults() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (_, ann) = app.user(&admin, "ann", json!({})).await;
    let (token, _) = app.app_token(ann, "library play").await;

    let set = app
        .put(DEFAULTS, &token, json!({"audio_language": "fra", "subtitle_language": "en", "subtitle_mode": "foreign"}))
        .await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.text());
    // The profile as GET /api/auth/me shows it to the app.
    assert_eq!(set.body, app.get("/api/auth/me", &token).await.body);
    assert_eq!(
        (&set.body["id"], &set.body["scope"], &set.body["audio_language"], &set.body["subtitle_mode"]),
        (&json!(ann), &json!("library play"), &json!("fra"), &json!("foreign"))
    );
    assert_eq!(stored(&app, ann).await, defaults(Some("fra"), Some("en"), "foreign"));

    // All three at once: a language left out is null, as the app sends a
    // null; a mode left out is the default one.
    let set = app.put(DEFAULTS, &token, json!({"subtitle_mode": "always"})).await;
    assert_eq!((set.status, &set.body["audio_language"]), (StatusCode::OK, &Value::Null));
    assert_eq!(stored(&app, ann).await, defaults(None, None, "always"));
    let set = app.put(DEFAULTS, &token, json!({"audio_language": "deu", "subtitle_language": null})).await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.text());
    assert_eq!(stored(&app, ann).await, defaults(Some("deu"), None, "forced"));

    // PATCH /api/me's rules for languages; refused, nothing changes.
    for (body, message) in [
        (json!({"audio_language": "EN"}), "audio_language must be an ISO 639 code such as \"eng\""),
        (json!({"audio_language": "english"}), "audio_language must be an ISO 639 code such as \"eng\""),
        (json!({"subtitle_language": ""}), "subtitle_language must be an ISO 639 code such as \"eng\""),
        (json!({"subtitle_language": "e1"}), "subtitle_language must be an ISO 639 code such as \"eng\""),
        (json!({"subtitle_mode": "sometimes"}), "subtitle_mode is off, forced, foreign or always"),
        (json!({"subtitle_mode": "unknown"}), "subtitle_mode is off, forced, foreign or always"),
    ] {
        let refused = app.put(DEFAULTS, &token, body.clone()).await;
        assert_eq!((refused.status, &refused.body["message"]), (StatusCode::BAD_REQUEST, &json!(message)), "{body}");
    }
    assert_eq!(stored(&app, ann).await, defaults(Some("deu"), None, "forced"));
    // Only its own profile's.
    assert_eq!(stored(&app, 1).await, defaults(None, None, "forced"));
}

#[tokio::test]
async fn an_administrators_app_is_answered_as_an_app() {
    let app = TestApp::new().await;
    app.setup_admin().await;
    let (token, _) = app.app_token(1, "play").await;
    let set = app.put(DEFAULTS, &token, json!({"subtitle_mode": "off"})).await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.text());
    assert_eq!((&set.body["is_admin"], &set.body["scope"]), (&json!(false), &json!("play")));
}

#[tokio::test]
async fn track_defaults_need_play() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (_, ann) = app.user(&admin, "ann", json!({})).await;
    for scope in ["library", "library playlists profiles"] {
        let (token, _) = app.app_token(ann, scope).await;
        let refused = app.put(DEFAULTS, &token, json!({"subtitle_mode": "always"})).await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN, "{scope}");
        assert_eq!(refused.headers[header::WWW_AUTHENTICATE], r#"Bearer error="insufficient_scope", scope="play""#);
        assert_eq!(refused.body["code"], "insufficient_scope");
        assert_eq!(
            refused.body["message"],
            "this app wasn't allowed to play titles, and keep the profile's place, watched marks, ratings and track \
             choices: sign in to it again to allow it"
        );
    }
    assert_eq!(stored(&app, ann).await, defaults(None, None, "forced"));
}

#[tokio::test]
async fn the_web_keeps_patch_and_apps_dont_get_it() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (session, ann) = app.user(&admin, "ann", json!({})).await;

    // A session changes all of it with PATCH /api/me, as before.
    let patched = app
        .patch("/api/me", &session, json!({"display_name": "Ann", "audio_language": "spa", "subtitle_mode": "always"}))
        .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.text());
    assert_eq!((&patched.body["display_name"], &patched.body["audio_language"]), (&json!("Ann"), &json!("spa")));
    assert_eq!(stored(&app, ann).await, defaults(Some("spa"), None, "always"));
    // And may use the new route too: the profile, without a scope.
    let put = app.put(DEFAULTS, &session, json!({"audio_language": "ita"})).await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.text());
    assert!(put.body.get("scope").is_none(), "{}", put.text());
    assert_eq!(stored(&app, ann).await, defaults(Some("ita"), None, "forced"));

    // An app's token, whatever it was allowed, never changes the name or
    // picture: PATCH /api/me stays the web's.
    let (token, _) = app.app_token(ann, "library play playlists profiles").await;
    let refused = app.patch("/api/me", &token, json!({"display_name": "Mallory", "audio_language": "eng"})).await;
    assert_eq!((refused.status, &refused.body["code"]), (StatusCode::FORBIDDEN, &json!("insufficient_scope")));
    assert_eq!(refused.body["message"], "apps can't do that: use pley's web app");
    let name: Option<String> = sqlx::query_scalar("SELECT display_name FROM users WHERE id = ?")
        .bind(ann)
        .fetch_one(&app.state.db.read)
        .await
        .unwrap();
    assert_eq!(name.as_deref(), Some("Ann"));
    assert_eq!(stored(&app, ann).await, defaults(Some("ita"), None, "forced"));
}
