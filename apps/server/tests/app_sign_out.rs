//! Every way pley signs an account out also ends, at once, the apps acting
//! as it or allowed by it: a password set by an administrator, signing out
//! everywhere, deleting the account and `pley reset-password`. Disabling an
//! account pauses its apps, and enabling it brings them back.

mod common;

use axum::http::{Method, StatusCode};
use pley::auth::grants::{RefreshError, RefreshInput};
use serde_json::json;

use common::TestApp;

/// Whether the app's next request gets through.
async fn works(app: &TestApp, access: &str) -> bool {
    let me = app.app_call(Method::GET, "/api/auth/me", access, None).await;
    match me.status {
        StatusCode::OK => true,
        StatusCode::UNAUTHORIZED => false,
        other => panic!("unexpected {other}: {:?}", me.body),
    }
}

/// Tries the app's refresh token: whether it gave a new pair.
async fn refreshes(app: &TestApp, refresh: &str) -> bool {
    let input = RefreshInput {
        token: refresh.to_owned(),
        client_id: common::ANDROID_APP.into(),
        scope: None,
        user_agent: None,
        ip: None,
    };
    match app.state.auth.refresh(input).await {
        Ok(_) => true,
        Err(RefreshError::Invalid(_)) => false,
        Err(other) => panic!("expected an invalid grant, got {other:?}"),
    }
}

/// The app ended: its next request answers 401, its refresh is refused.
async fn assert_ended(app: &TestApp, (access, refresh): &(String, String)) {
    assert!(!works(app, access).await, "the access token still works");
    assert!(!refreshes(app, refresh).await, "the refresh token still works");
}

async fn last_grant(app: &TestApp) -> i64 {
    sqlx::query_scalar("SELECT id FROM app_grants ORDER BY id DESC LIMIT 1")
        .fetch_one(&app.state.db.read)
        .await
        .unwrap()
}

#[tokio::test]
async fn an_administrator_setting_a_password_ends_the_accounts_apps() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (_bob, bob) = app.user(&admin, "bob", json!({})).await;
    let tokens = app.app_token(bob, "library play").await;
    assert!(works(&app, &tokens.0).await);

    let set = app.patch(&format!("/api/users/{bob}"), &admin, json!({"password": "another-pass-2"})).await;
    assert_eq!(set.status, StatusCode::OK, "{:?}", set.body);
    assert_ended(&app, &tokens).await;

    // An administrator setting their own password through the users page
    // keeps their session, but their apps end too.
    let me: i64 = app.get("/api/auth/me", &admin).await.body["id"].as_i64().unwrap();
    let mine = app.app_token(me, "library").await;
    let set = app.patch(&format!("/api/users/{me}"), &admin, json!({"password": "secret-two"})).await;
    assert_eq!(set.status, StatusCode::OK, "{:?}", set.body);
    assert_eq!(app.get("/api/auth/me", &admin).await.status, StatusCode::OK, "the session stays");
    assert_ended(&app, &mine).await;
}

#[tokio::test]
async fn signing_out_everywhere_ends_apps_too() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (session, bob) = app.user(&admin, "bob", json!({})).await;
    let tokens = app.app_token(bob, "library play").await;
    assert!(works(&app, &tokens.0).await);

    let out = app.delete(&format!("/api/users/{bob}/devices"), &admin).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT, "{:?}", out.body);
    assert_eq!(app.get("/api/auth/me", &session).await.status, StatusCode::UNAUTHORIZED, "the session ends");
    assert_ended(&app, &tokens).await;
}

#[tokio::test]
async fn deleting_an_account_ends_its_apps_at_once() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (_bob, bob) = app.user(&admin, "bob", json!({})).await;
    let tokens = app.app_token(bob, "library play").await;
    // Cached now: the deletion must drop it, not wait for it to lapse.
    assert!(works(&app, &tokens.0).await);

    assert_eq!(app.delete(&format!("/api/users/{bob}"), &admin).await.status, StatusCode::NO_CONTENT);
    assert_ended(&app, &tokens).await;
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_grants").fetch_one(&app.state.db.read).await.unwrap();
    assert_eq!(left, 0, "the grant went with the account");
}

#[tokio::test]
async fn reset_password_ends_the_accounts_apps() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (_bob, bob) = app.user(&admin, "bob", json!({})).await;
    // Never used, so nothing is cached: as with the server closed.
    let tokens = app.app_token(bob, "library play").await;

    pley::reset_password(app.state.config.clone(), "bob", "another-pass-2", false).await.unwrap();
    assert_ended(&app, &tokens).await;
}

#[tokio::test]
async fn disabling_an_account_pauses_its_apps_and_enabling_brings_them_back() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (_bob, bob) = app.user(&admin, "bob", json!({})).await;
    let (access, refresh) = app.app_token(bob, "library play").await;
    assert!(works(&app, &access).await);

    let off = app.patch(&format!("/api/users/{bob}"), &admin, json!({"disabled": true})).await;
    assert_eq!(off.status, StatusCode::OK, "{:?}", off.body);
    assert!(!works(&app, &access).await, "paused at once");
    assert!(!refreshes(&app, &refresh).await, "no refresh while disabled");

    let on = app.patch(&format!("/api/users/{bob}"), &admin, json!({"disabled": false})).await;
    assert_eq!(on.status, StatusCode::OK, "{:?}", on.body);
    assert!(works(&app, &access).await, "back, unchanged");
    assert!(refreshes(&app, &refresh).await, "the refused refresh spent nothing");
}

#[tokio::test]
async fn apps_allowed_by_someone_end_with_their_sign_ins() {
    // Ana allowed an app that acts as Bo (a household profile switch does
    // this); what happens to Ana's sign-ins happens to it.
    async fn allowed_by_ana(app: &TestApp, ana: i64, bo: i64) -> (String, String) {
        let tokens = app.app_token(ana, "library play").await;
        let grant = last_grant(app).await;
        sqlx::query("UPDATE app_grants SET user_id = ? WHERE id = ?")
            .bind(bo)
            .bind(grant)
            .execute(&app.state.db.write)
            .await
            .unwrap();
        // Nothing cached from before the move.
        app.state.auth.forget_grants(&[grant]);
        assert!(works(app, &tokens.0).await);
        let me = app.app_call(Method::GET, "/api/auth/me", &tokens.0, None).await;
        assert_eq!(me.body["id"].as_i64(), Some(bo), "it acts as Bo");
        tokens
    }

    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (_ana, ana) = app.user(&admin, "ana", json!({})).await;
    let (_bo, bo) = app.user(&admin, "bo", json!({})).await;

    // Ana's password set by an administrator.
    let tokens = allowed_by_ana(&app, ana, bo).await;
    let set = app.patch(&format!("/api/users/{ana}"), &admin, json!({"password": "another-pass-2"})).await;
    assert_eq!(set.status, StatusCode::OK, "{:?}", set.body);
    assert_ended(&app, &tokens).await;

    // Ana signed out everywhere.
    let tokens = allowed_by_ana(&app, ana, bo).await;
    let out = app.delete(&format!("/api/users/{ana}/devices"), &admin).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT, "{:?}", out.body);
    assert_ended(&app, &tokens).await;

    // Ana disabled: paused, and back when she is enabled again.
    let (access, refresh) = allowed_by_ana(&app, ana, bo).await;
    assert_eq!(app.patch(&format!("/api/users/{ana}"), &admin, json!({"disabled": true})).await.status, StatusCode::OK);
    assert!(!works(&app, &access).await);
    assert!(!refreshes(&app, &refresh).await);
    assert_eq!(
        app.patch(&format!("/api/users/{ana}"), &admin, json!({"disabled": false})).await.status,
        StatusCode::OK
    );
    assert!(works(&app, &access).await);

    // Ana's account deleted.
    assert_eq!(app.delete(&format!("/api/users/{ana}"), &admin).await.status, StatusCode::NO_CONTENT);
    assert_ended(&app, &(access, refresh)).await;
    assert_eq!(app.get("/api/auth/me", &app.login("bo", "test-pass-1").await).await.status, StatusCode::OK, "Bo stays");
}
