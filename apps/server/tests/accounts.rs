//! Accounts end to end (docs/API.md: Auth, Your account, Users,
//! Invitations): who may manage users, the rules that keep the server
//! usable, PINs and profile switching, invitations, devices and login
//! throttling.

mod common;

use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};

use common::{Reply, TestApp};

async fn user_named(app: &TestApp, admin: &str, username: &str) -> Value {
    let users = app.get("/api/users", admin).await;
    assert_eq!(users.status, StatusCode::OK);
    users.body.as_array().unwrap().iter().find(|u| u["username"] == username).cloned().expect(username)
}

async fn switch(app: &TestApp, token: &str, body: Value) -> Reply {
    app.post("/api/auth/switch", token, body).await
}

async fn accept(app: &TestApp, code: &str, body: Value) -> Reply {
    app.call(Method::POST, &format!("/api/invites/{code}/accept"), None, Some(body), &[]).await
}

async fn uses_left(app: &TestApp, admin: &str, code: &str) -> Option<Value> {
    let list = app.get("/api/invites", admin).await;
    list.body.as_array().unwrap().iter().find(|i| i["code"] == code).map(|i| i["uses_left"].clone())
}

async fn change_password(app: &TestApp, token: &str, current: &str, new: &str) -> Reply {
    app.put("/api/auth/password", token, json!({"current_password": current, "new_password": new})).await
}

#[tokio::test]
async fn pley_always_keeps_an_enabled_administrator() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let admin_id = app.get("/api/auth/me", &admin).await.body["id"].as_i64().unwrap();

    let demote = app.patch(&format!("/api/users/{admin_id}"), &admin, json!({"is_admin": false})).await;
    assert_eq!(demote.status, StatusCode::BAD_REQUEST, "the only administrator");
    let disable = app.patch(&format!("/api/users/{admin_id}"), &admin, json!({"disabled": true})).await;
    assert_eq!(disable.status, StatusCode::BAD_REQUEST, "nobody disables their own account");

    // A second administrator who is disabled doesn't count.
    let (_, second_id) = app.user(&admin, "second", json!({"is_admin": true})).await;
    let disabled = app.patch(&format!("/api/users/{second_id}"), &admin, json!({"disabled": true})).await;
    assert_eq!(disabled.status, StatusCode::OK, "{:?}", disabled.body);
    let demote = app.patch(&format!("/api/users/{admin_id}"), &admin, json!({"is_admin": false})).await;
    assert_eq!(demote.status, StatusCode::BAD_REQUEST);

    // Once it is enabled again, the first one may step down, right away.
    app.patch(&format!("/api/users/{second_id}"), &admin, json!({"disabled": false})).await;
    let second = app.login("second", "password1").await;
    let demote = app.patch(&format!("/api/users/{admin_id}"), &admin, json!({"is_admin": false})).await;
    assert_eq!(demote.status, StatusCode::OK, "{:?}", demote.body);
    assert_eq!(app.get("/api/users", &admin).await.status, StatusCode::FORBIDDEN);
    let me_now = app.patch(&format!("/api/users/{second_id}"), &second, json!({"is_admin": false})).await;
    assert_eq!(me_now.status, StatusCode::BAD_REQUEST, "now the only one");

    // Managed profiles can't be administrators.
    let kid = app.post("/api/users", &second, json!({"username": "kid", "managed": true})).await;
    let kid_id = kid.body["id"].as_i64().unwrap();
    let promote = app.patch(&format!("/api/users/{kid_id}"), &second, json!({"is_admin": true})).await;
    assert_eq!(promote.status, StatusCode::BAD_REQUEST);
    let both = app.post("/api/users", &second, json!({"username": "kid2", "managed": true, "is_admin": true})).await;
    assert_eq!(both.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_rejected_user_update_changes_nothing() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (bob, bob_id) = app.user(&admin, "bob", json!({})).await;

    let bad_age =
        app.patch(&format!("/api/users/{bob_id}"), &admin, json!({"display_name": "Robert", "max_age": 99})).await;
    assert_eq!(bad_age.status, StatusCode::BAD_REQUEST);
    assert_eq!(user_named(&app, &admin, "bob").await["display_name"], Value::Null);

    let bad_pin =
        app.patch(&format!("/api/users/{bob_id}"), &admin, json!({"password": "brand-new", "pin": "12"})).await;
    assert_eq!(bad_pin.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.get("/api/auth/me", &bob).await.status, StatusCode::OK, "bob is still signed in");
    assert_eq!(app.try_login("bob", "password1").await.status, StatusCode::OK, "with his old password");

    let admin_id = app.get("/api/auth/me", &admin).await.body["id"].as_i64().unwrap();
    let last =
        app.patch(&format!("/api/users/{admin_id}"), &admin, json!({"display_name": "Boss", "is_admin": false})).await;
    assert_eq!(last.status, StatusCode::BAD_REQUEST);
    assert_eq!(user_named(&app, &admin, "admin").await["display_name"], Value::Null);
}

#[tokio::test]
async fn a_new_password_from_an_administrator_signs_the_user_out() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (laptop, bob_id) = app.user(&admin, "bob", json!({})).await;
    let phone = app.login("bob", "password1").await;

    let reset = app.patch(&format!("/api/users/{bob_id}"), &admin, json!({"password": "fresh-start"})).await;
    assert_eq!(reset.status, StatusCode::OK, "{:?}", reset.body);
    for token in [&laptop, &phone] {
        assert_eq!(app.get("/api/auth/me", token).await.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(app.try_login("bob", "password1").await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.try_login("bob", "fresh-start").await.status, StatusCode::OK);

    // A managed profile given a password becomes a regular account.
    let kid = app.post("/api/users", &admin, json!({"username": "kid", "managed": true})).await;
    let kid_id = kid.body["id"].as_i64().unwrap();
    assert_eq!(app.try_login("kid", "!managed").await.status, StatusCode::UNAUTHORIZED);
    let grown = app.patch(&format!("/api/users/{kid_id}"), &admin, json!({"password": "grown-up"})).await;
    assert_eq!(grown.body["managed"], json!(false));
    assert_eq!(app.try_login("kid", "grown-up").await.status, StatusCode::OK);

    // Administrators resetting their own password stay signed in.
    let admin_id = app.get("/api/auth/me", &admin).await.body["id"].as_i64().unwrap();
    app.patch(&format!("/api/users/{admin_id}"), &admin, json!({"password": "secret2"})).await;
    assert_eq!(app.get("/api/auth/me", &admin).await.status, StatusCode::OK);
}

#[tokio::test]
async fn new_users_are_checked() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.media_file("movies/Film (2001).mkv", 1024);
    let movies = app.library(&admin, "Movies", "movies", "movies").await;

    let create = |body: Value| app.post("/api/users", &admin, body);
    assert_eq!(create(json!({"username": "ADMIN", "password": "password1"})).await.status, StatusCode::CONFLICT);
    for body in [
        json!({"username": "bad name", "password": "password1"}),
        json!({"username": "", "password": "password1"}),
        json!({"username": "nopass"}),
        json!({"username": "short", "password": "abc"}),
        json!({"username": "old", "password": "password1", "max_age": 22}),
        json!({"username": "young", "password": "password1", "max_age": -1}),
        json!({"username": "pin", "password": "password1", "pin": "12"}),
    ] {
        let reply = create(body.clone()).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{body}: {:?}", reply.body);
    }

    let created = create(json!({
        "username": "  carol  ",
        "password": "password1",
        "display_name": "   ",
        "all_libraries": false,
        "library_ids": [movies, 999],
        "max_age": 0,
    }))
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    assert_eq!(created.body["username"], json!("carol"));
    assert_eq!((created.body["name"].clone(), created.body["display_name"].clone()), (json!("carol"), Value::Null));
    assert_eq!(created.body["library_ids"], json!([movies]), "unknown libraries are dropped");
    assert_eq!((created.body["max_age"].clone(), created.body["home"].clone()), (json!(0), json!(true)));

    // None of this is open to other users.
    let carol = app.login("carol", "password1").await;
    let carol_id = created.body["id"].as_i64().unwrap();
    assert_eq!(app.get("/api/users", &carol).await.status, StatusCode::FORBIDDEN);
    assert_eq!(create_as(&app, &carol).await, StatusCode::FORBIDDEN);
    let selfish = app.patch(&format!("/api/users/{carol_id}"), &carol, json!({"max_age": null})).await;
    assert_eq!(selfish.status, StatusCode::FORBIDDEN);
    assert_eq!(app.delete(&format!("/api/users/{carol_id}"), &carol).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.get(&format!("/api/users/{carol_id}/devices"), &carol).await.status, StatusCode::FORBIDDEN);
}

async fn create_as(app: &TestApp, token: &str) -> StatusCode {
    app.post("/api/users", token, json!({"username": "eve", "password": "password1", "is_admin": true})).await.status
}

#[tokio::test]
async fn deleting_a_user_signs_them_out_and_removes_their_picture() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let admin_id = app.get("/api/auth/me", &admin).await.body["id"].as_i64().unwrap();
    let (bob, bob_id) = app.user(&admin, "bob", json!({})).await;
    let mut png = Vec::new();
    image::RgbImage::from_pixel(32, 32, image::Rgb([10, 20, 30]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let req = axum::http::Request::builder()
        .method(Method::PUT)
        .uri("/api/me/avatar")
        .header(header::AUTHORIZATION, format!("Bearer {bob}"))
        .body(axum::body::Body::from(png))
        .unwrap();
    let uploaded = tower::ServiceExt::oneshot(app.api.clone(), req).await.unwrap();
    assert_eq!(uploaded.status(), StatusCode::OK);
    let avatars = app.state.config.data_dir.join("avatars");
    assert_eq!(std::fs::read_dir(&avatars).unwrap().count(), 1);

    assert_eq!(app.delete(&format!("/api/users/{admin_id}"), &admin).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.delete("/api/users/999", &admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.delete(&format!("/api/users/{bob_id}"), &admin).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/auth/me", &bob).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.try_login("bob", "password1").await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(std::fs::read_dir(&avatars).unwrap().count(), 0, "the picture is gone too");
    assert_eq!(app.get(&format!("/api/users/{bob_id}/avatar"), &admin).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn your_own_pin_needs_your_password() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (parent, parent_id) = app.user(&admin, "parent", json!({})).await;

    let set_pin = |body: Value| app.put("/api/me/pin", &parent, body);
    assert_eq!(set_pin(json!({"pin": "4321"})).await.status, StatusCode::BAD_REQUEST, "no password");
    assert_eq!(set_pin(json!({"pin": "4321", "password": "nope"})).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(set_pin(json!({"pin": "12", "password": "password1"})).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(set_pin(json!({"pin": "4321", "password": "password1"})).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/auth/me", &parent).await.body["has_pin"], json!(true));

    // Switching to the profile now takes the PIN, even for an administrator.
    let without = app.post("/api/auth/switch", &admin, json!({"user_id": parent_id})).await;
    assert_eq!(without.status, StatusCode::FORBIDDEN);
    let with = app.post("/api/auth/switch", &admin, json!({"user_id": parent_id, "pin": "4321"})).await;
    assert_eq!(with.status, StatusCode::OK, "{:?}", with.body);

    assert_eq!(set_pin(json!({"pin": null, "password": "password1"})).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/auth/me", &parent).await.body["has_pin"], json!(false));

    // Managed profiles have no password: an administrator sets their PIN.
    let kid =
        app.post("/api/users", &app.login("admin", "secret1").await, json!({"username": "kid", "managed": true})).await;
    let kid_id = kid.body["id"].as_i64().unwrap();
    let kid = app.post("/api/auth/switch", &parent, json!({"user_id": kid_id})).await.body["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let refused = app.put("/api/me/pin", &kid, json!({"pin": "1111", "password": ""})).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn who_can_switch_to_whom() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let admin_id = app.get("/api/auth/me", &admin).await.body["id"].as_i64().unwrap();
    let (friend, friend_id) = app.user(&admin, "friend", json!({"home": false})).await;
    let kid =
        app.post("/api/users", &admin, json!({"username": "kid", "managed": true})).await.body["id"].as_i64().unwrap();
    let teen =
        app.post("/api/users", &admin, json!({"username": "teen", "managed": true})).await.body["id"].as_i64().unwrap();
    let away =
        app.post("/api/users", &admin, json!({"username": "away", "managed": true})).await.body["id"].as_i64().unwrap();
    app.patch(&format!("/api/users/{away}"), &admin, json!({"disabled": true})).await;

    // Friends are not part of the household, disabled profiles are not offered.
    assert_eq!(app.get("/api/auth/profiles", &friend).await.body, json!([]));
    let profiles = app.get("/api/auth/profiles", &admin).await;
    let names: Vec<&str> = profiles.body.as_array().unwrap().iter().map(|u| u["username"].as_str().unwrap()).collect();
    assert_eq!(names, ["admin", "kid", "teen"]);
    assert_eq!(switch(&app, &admin, json!({"user_id": friend_id})).await.status, StatusCode::FORBIDDEN);
    let to_admin = switch(&app, &friend, json!({"user_id": admin_id, "password": "secret1"})).await;
    assert_eq!(to_admin.status, StatusCode::FORBIDDEN, "friends can't switch at all");
    assert_eq!(switch(&app, &admin, json!({"user_id": away})).await.status, StatusCode::FORBIDDEN);
    assert_eq!(switch(&app, &admin, json!({"user_id": 999})).await.status, StatusCode::NOT_FOUND);

    // Profiles without a PIN are open between household members.
    let kid_token = switch(&app, &admin, json!({"user_id": kid})).await.body["token"].as_str().unwrap().to_owned();
    let teen_token = switch(&app, &kid_token, json!({"user_id": teen})).await;
    assert_eq!(teen_token.status, StatusCode::OK);
    let teen_token = teen_token.body["token"].as_str().unwrap().to_owned();
    assert_eq!(app.get("/api/auth/me", &kid_token).await.status, StatusCode::UNAUTHORIZED, "one profile per device");
    // Switching to yourself just renews the session.
    let again = switch(&app, &teen_token, json!({"user_id": teen})).await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(app.get("/api/auth/me", &teen_token).await.status, StatusCode::UNAUTHORIZED);
    let teen_token = again.body["token"].as_str().unwrap().to_owned();

    // An administrator with a PIN asks for the PIN, not the password.
    let admin = app.login("admin", "secret1").await;
    app.patch(&format!("/api/users/{admin_id}"), &admin, json!({"pin": "2468"})).await;
    let by_password = switch(&app, &teen_token, json!({"user_id": admin_id, "password": "secret1"})).await;
    assert_eq!(by_password.status, StatusCode::FORBIDDEN);
    let by_pin = switch(&app, &teen_token, json!({"user_id": admin_id, "pin": "2468"})).await;
    assert_eq!(by_pin.status, StatusCode::OK, "{:?}", by_pin.body);
    assert_eq!(by_pin.body["user"]["is_admin"], json!(true));
}

#[tokio::test]
async fn invitations() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.media_file("movies/Film (2001).mkv", 1024);
    let movies = app.library(&admin, "Movies", "movies", "movies").await;
    let invite = |body: Value| app.post("/api/invites", &admin, body);

    assert_eq!(invite(json!({"max_age": 30})).await.status, StatusCode::BAD_REQUEST);
    let created = invite(json!({
        "uses": 2,
        "expires_in_days": 0,
        "all_libraries": false,
        "library_ids": [movies],
        "max_age": 12,
        "note": format!("  {}  ", "n".repeat(300)),
    }))
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    assert_eq!(created.body["expires_at"], Value::Null, "0 days: never expires");
    assert_eq!(created.body["note"].as_str().unwrap().len(), 200);
    let code = created.body["code"].as_str().unwrap().to_owned();
    let check = app.call(Method::GET, &format!("/api/invites/{code}"), None, None, &[]).await;
    assert_eq!(check.status, StatusCode::OK);

    // Refused sign-ups don't use the invitation up.
    let bad = accept(&app, &code, json!({"username": "bad name", "password": "password1"})).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let taken = accept(&app, &code, json!({"username": "Admin", "password": "password1"})).await;
    assert_eq!(taken.status, StatusCode::CONFLICT);
    assert_eq!(uses_left(&app, &admin, &code).await, Some(json!(2)));

    let first = accept(&app, &code, json!({"username": "ann", "password": "password1", "display_name": " Ann "})).await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.body);
    let user = &first.body["user"];
    assert_eq!(
        (user["name"].clone(), user["home"].clone(), user["max_age"].clone()),
        (json!("Ann"), json!(false), json!(12))
    );
    assert_eq!(user["library_ids"], json!([movies]));
    assert!(first.headers[header::SET_COOKIE].to_str().unwrap().starts_with("pley_token="));
    let second = accept(&app, &code, json!({"username": "ben", "password": "password1"})).await;
    assert_eq!(second.status, StatusCode::OK);
    let third = accept(&app, &code, json!({"username": "cid", "password": "password1"})).await;
    assert_eq!(third.status, StatusCode::NOT_FOUND);
    let check = app.call(Method::GET, &format!("/api/invites/{code}"), None, None, &[]).await;
    assert_eq!(check.status, StatusCode::NOT_FOUND, "used up");

    // Expired and deleted invitations.
    let week = invite(json!({})).await.body["code"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE invites SET expires_at = 1 WHERE code = ?")
        .bind(&week)
        .execute(&app.state.db.write)
        .await
        .unwrap();
    let late = accept(&app, &week, json!({"username": "dan", "password": "password1"})).await;
    assert_eq!(late.status, StatusCode::NOT_FOUND);
    let gone = invite(json!({})).await.body["code"].as_str().unwrap().to_owned();
    assert_eq!(app.delete(&format!("/api/invites/{gone}"), &admin).await.status, StatusCode::NO_CONTENT);
    let deleted = accept(&app, &gone, json!({"username": "eve", "password": "password1"})).await;
    assert_eq!(deleted.status, StatusCode::NOT_FOUND);

    // Only administrators manage invitations.
    let ann = app.login("ann", "password1").await;
    assert_eq!(app.get("/api/invites", &ann).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.post("/api/invites", &ann, json!({})).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.delete(&format!("/api/invites/{week}"), &ann).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn devices_belong_to_their_user() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let admin_id = app.get("/api/auth/me", &admin).await.body["id"].as_i64().unwrap();
    let (bob, bob_id) = app.user(&admin, "bob", json!({})).await;
    let (alice, _) = app.user(&admin, "alice", json!({})).await;

    let bobs = app.get(&format!("/api/users/{bob_id}/devices"), &admin).await;
    assert_eq!(bobs.body.as_array().unwrap().len(), 1);
    assert_eq!(bobs.body[0]["current"], json!(false), "nobody's current device in the admin view");
    let device = bobs.body[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(device.len(), 16);

    for id in [device.as_str(), "not-hex-at-all-x", "abc"] {
        let reply = app.delete(&format!("/api/me/devices/{id}"), &alice).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{id}");
    }
    assert_eq!(app.get("/api/auth/me", &bob).await.status, StatusCode::OK, "alice can't sign bob out");

    assert_eq!(app.delete(&format!("/api/users/{admin_id}/devices"), &admin).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.delete(&format!("/api/users/{bob_id}/devices"), &admin).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/auth/me", &bob).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.get(&format!("/api/users/{bob_id}/devices"), &admin).await.body, json!([]));
}

#[tokio::test]
async fn login_attempts_are_throttled_per_username() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.user(&admin, "bob", json!({})).await;
    app.user(&admin, "carol", json!({})).await;

    for _ in 0..5 {
        assert_eq!(app.try_login("bob", "wrong").await.status, StatusCode::UNAUTHORIZED);
    }
    let locked = app.try_login("bob", "password1").await;
    assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS, "even with the right password");
    assert_eq!(locked.body["code"], json!("too_many_requests"));
    assert_eq!(app.try_login(" BOB ", "password1").await.status, StatusCode::TOO_MANY_REQUESTS, "same account");
    assert_eq!(app.try_login("carol", "password1").await.status, StatusCode::OK, "others are not affected");

    // Unknown names are throttled the same way, so they look like real ones.
    for _ in 0..5 {
        assert_eq!(app.try_login("ghost", "wrong").await.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(app.try_login("ghost", "wrong").await.status, StatusCode::TOO_MANY_REQUESTS);

    // A successful sign-in starts the count again.
    for _ in 0..4 {
        app.try_login("carol", "wrong").await;
    }
    assert_eq!(app.try_login("carol", "password1").await.status, StatusCode::OK);
    for _ in 0..4 {
        app.try_login("carol", "wrong").await;
    }
    assert_eq!(app.try_login("carol", "password1").await.status, StatusCode::OK);
}

#[tokio::test]
async fn changing_your_password() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (bob, _) = app.user(&admin, "bob", json!({})).await;
    let short = change_password(&app, &bob, "password1", "abc").await;
    assert_eq!(short.status, StatusCode::BAD_REQUEST, "too short");
    assert_eq!(change_password(&app, &bob, "wrong", "password2").await.status, StatusCode::BAD_REQUEST);
    assert_eq!(change_password(&app, &bob, "password1", "password2").await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.try_login("bob", "password1").await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.try_login("bob", "password2").await.status, StatusCode::OK);

    let kid = app.post("/api/users", &admin, json!({"username": "kid", "managed": true})).await.body["id"].clone();
    let kid = switch(&app, &admin, json!({"user_id": kid})).await.body["token"].as_str().unwrap().to_owned();
    let managed = change_password(&app, &kid, "!managed", "password3").await;
    assert_eq!(managed.status, StatusCode::BAD_REQUEST);
    assert!(managed.body["message"].as_str().unwrap().contains("managed"));
}

#[tokio::test]
async fn setup_checks_the_first_account_and_logout_clears_the_cookie() {
    let app = TestApp::new().await;
    for body in
        [json!({"username": "bad name", "password": "secret1"}), json!({"username": "admin", "password": "abc"})]
    {
        let reply = app.call(Method::POST, "/api/setup", None, Some(body), &[]).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    }
    let server = app.call(Method::GET, "/api/server", None, None, &[]).await;
    assert_eq!(server.body["setup_required"], json!(true));
    let admin = app.setup_admin().await;
    let me = app.get("/api/auth/me", &admin).await;
    assert_eq!((me.body["is_admin"].clone(), me.body["home"].clone()), (json!(true), json!(true)));

    let out = app.call(Method::POST, "/api/auth/logout", Some(&admin), None, &[]).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    let cookie = out.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.starts_with("pley_token=;") && cookie.contains("Max-Age=0"), "{cookie}");
    // Signing out without a session is fine too.
    assert_eq!(app.call(Method::POST, "/api/auth/logout", None, None, &[]).await.status, StatusCode::NO_CONTENT);
}
