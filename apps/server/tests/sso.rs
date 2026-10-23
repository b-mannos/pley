//! Single sign-on against a stand-in OpenID Connect provider: discovery,
//! PKCE, the state bound to the browser, ID token checks, linking, new
//! accounts, the group that may sign in, the scopes pley asks for and the
//! client metadata document.

mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Form, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use parking_lot::Mutex;
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use common::{Reply, TestApp};

const PUBLIC: &str = "https://pley.example.com";
const HOST: &str = "pley.example.com";

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

/// A new Ed25519 key, as PKCS#8 (made for each run: no keys in the repository).
fn new_key() -> Vec<u8> {
    Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap().as_ref().to_vec()
}

/// What the stand-in provider does when pley redeems a code.
#[derive(Clone, Default)]
struct Grant {
    challenge: String,
    redirect_uri: String,
    client_id: String,
    /// The ID token's claims, over the usual ones (`iss`, `aud`, `sub`,
    /// `iat`, `exp`); `null` removes one.
    claims: Value,
    /// Signs with HS256 and this secret instead of the provider's key.
    secret: Option<String>,
    /// Signs with this key instead (PKCS#8), as `kid`.
    key: Option<(Vec<u8>, String)>,
}

#[derive(Clone)]
struct Idp {
    issuer: String,
    pkcs8: Arc<Vec<u8>>,
    grants: Arc<Mutex<HashMap<String, Grant>>>,
    /// What the userinfo endpoint says.
    userinfo: Arc<Mutex<Value>>,
    /// The client credentials the token endpoint wants in HTTP Basic.
    basic: Arc<Mutex<Option<(String, String)>>>,
    cimd: bool,
}

impl Idp {
    fn grant(&self, code: &str, grant: Grant) {
        self.grants.lock().insert(code.to_owned(), grant);
    }
}

async fn discovery(State(idp): State<Idp>) -> Json<Value> {
    let base = &idp.issuer;
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "userinfo_endpoint": format!("{base}/userinfo"),
        "jwks_uri": format!("{base}/jwks"),
        "id_token_signing_alg_values_supported": ["EdDSA", "HS256"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "none"],
        "client_id_metadata_document_supported": idp.cimd,
    }))
}

async fn jwks(State(idp): State<Idp>) -> Json<Value> {
    let key = Ed25519KeyPair::from_pkcs8(&idp.pkcs8).unwrap();
    Json(json!({"keys": [
        // Keys pley can't use don't spoil the ones it can.
        {"kty": "EC", "crv": "P-521", "alg": "ES512", "kid": "big", "x": "AQ", "y": "AQ"},
        {"kty": "RSA", "use": "enc", "alg": "RSA-OAEP", "kid": "enc", "n": "AQAB", "e": "AQAB"},
        {"kty": "OKP", "crv": "Ed25519", "use": "sig", "alg": "EdDSA", "kid": "k1",
         "x": URL_SAFE_NO_PAD.encode(key.public_key().as_ref())},
    ]}))
}

fn token_error(error: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error": error, "error_description": "the stand-in said no"})))
        .into_response()
}

async fn token(State(idp): State<Idp>, headers: HeaderMap, Form(form): Form<HashMap<String, String>>) -> Response {
    let Some(grant) = form.get("code").and_then(|code| idp.grants.lock().remove(code)) else {
        return token_error("invalid_grant");
    };
    let verifier = form.get("code_verifier").cloned().unwrap_or_default();
    if URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != grant.challenge {
        return token_error("invalid_grant");
    }
    if form.get("redirect_uri") != Some(&grant.redirect_uri)
        || form.get("grant_type").map(String::as_str) != Some("authorization_code")
    {
        return token_error("invalid_request");
    }
    let basic = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(|v| STANDARD.decode(v).ok())
        .and_then(|v| String::from_utf8(v).ok());
    let client = match idp.basic.lock().clone() {
        Some((id, secret)) if basic == Some(format!("{id}:{secret}")) => id,
        Some(_) => return token_error("invalid_client"),
        None => form.get("client_id").cloned().unwrap_or_default(),
    };
    if client != grant.client_id {
        return token_error("invalid_client");
    }
    let mut claims = json!({
        "iss": idp.issuer, "aud": grant.client_id, "sub": "ann-at-the-provider", "iat": now(), "exp": now() + 300,
    });
    for (name, value) in grant.claims.as_object().cloned().unwrap_or_default() {
        if value.is_null() {
            claims.as_object_mut().unwrap().remove(&name);
        } else {
            claims[name] = value;
        }
    }
    let id_token = match (&grant.secret, &grant.key) {
        (Some(secret), _) => {
            jsonwebtoken::encode(&Header::new(Algorithm::HS256), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        }
        (None, key) => {
            let (pkcs8, kid) = key.clone().unwrap_or(((*idp.pkcs8).clone(), "k1".into()));
            let mut header = Header::new(Algorithm::EdDSA);
            header.kid = Some(kid);
            jsonwebtoken::encode(&header, &claims, &EncodingKey::from_ed_der(&pkcs8))
        }
    }
    .unwrap();
    Json(json!({"id_token": id_token, "access_token": "access-1", "token_type": "Bearer", "expires_in": 300}))
        .into_response()
}

async fn userinfo(State(idp): State<Idp>, headers: HeaderMap) -> Response {
    if headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) != Some("Bearer access-1") {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(idp.userinfo.lock().clone()).into_response()
}

async fn start_idp(cimd: bool) -> Idp {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let idp = Idp {
        issuer: format!("http://{addr}"),
        pkcs8: Arc::new(new_key()),
        grants: Default::default(),
        userinfo: Arc::new(Mutex::new(json!({"sub": "ann-at-the-provider"}))),
        basic: Default::default(),
        cimd,
    };
    let app = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/token", post(token))
        .route("/userinfo", get(userinfo))
        .with_state(idp.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    idp
}

/// The value of the cookie `name` that a reply sets (`""` when it deletes it).
fn set_cookie(reply: &Reply, name: &str) -> Option<String> {
    reply.headers.get_all(header::SET_COOKIE).iter().filter_map(|v| v.to_str().ok()).find_map(|cookie| {
        let (pair, _) = cookie.split_once(';').unwrap_or((cookie, ""));
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| value.to_owned())
    })
}

fn location(reply: &Reply) -> String {
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.text());
    reply.headers[header::LOCATION].to_str().unwrap().to_owned()
}

/// A sign-in on its way to the provider: what pley sent it there with.
struct Started {
    query: HashMap<String, String>,
    cookie: String,
}

impl Started {
    fn get(&self, name: &str) -> &str {
        self.query.get(name).map(String::as_str).unwrap_or_default()
    }

    /// The provider lets the person in as `claims` say (the nonce comes along).
    fn grant(&self, idp: &Idp, code: &str, mut claims: Value) {
        if claims.get("nonce").is_none() {
            claims["nonce"] = json!(self.get("nonce"));
        }
        idp.grant(
            code,
            Grant {
                challenge: self.get("code_challenge").into(),
                redirect_uri: self.get("redirect_uri").into(),
                client_id: self.get("client_id").into(),
                claims,
                ..Default::default()
            },
        );
    }
}

fn with_cookies(cookies: &str) -> Vec<(&str, &str)> {
    let mut headers = vec![("host", HOST)];
    if !cookies.is_empty() {
        headers.push(("cookie", cookies));
    }
    headers
}

async fn start(app: &TestApp, idp: &Idp, cookies: &str) -> Started {
    let reply =
        app.call(Method::GET, "/api/auth/sso/start?next=/library/nckkvbd", None, None, &with_cookies(cookies)).await;
    let to = url::Url::parse(&location(&reply)).unwrap();
    assert!(to.as_str().starts_with(&format!("{}/authorize?", idp.issuer)), "{to}");
    let cookie = set_cookie(&reply, "pley_sso").filter(|c| !c.is_empty()).expect("the sign-in's cookie");
    Started { query: to.query_pairs().into_owned().collect(), cookie }
}

/// Back from the provider with `code`.
async fn back(app: &TestApp, started: &Started, code: &str, cookies: &str) -> Reply {
    let uri = format!("/api/auth/sso/callback?code={code}&state={}", started.get("state"));
    let cookies = format!("pley_sso={}; {cookies}", started.cookie);
    app.call(Method::GET, &uri, None, None, &with_cookies(cookies.trim_end_matches("; "))).await
}

/// pley with an administrator and single sign-on through `idp` with the
/// client `pley` and the secret `s3cret`, and a second user, `ann`.
async fn world(cimd: bool) -> (TestApp, Idp, String, String, i64) {
    let idp = start_idp(cimd).await;
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let settings = if cimd {
        json!({"public_url": PUBLIC, "sso_enabled": true, "sso_issuer": idp.issuer, "sso_name": "Stand-in"})
    } else {
        *idp.basic.lock() = Some(("pley".into(), "s3cret".into()));
        json!({"public_url": PUBLIC, "sso_enabled": true, "sso_issuer": idp.issuer, "sso_name": "Stand-in",
               "sso_client_id": "pley", "sso_client_secret": "s3cret"})
    };
    let saved = app.put("/api/settings", &admin, settings).await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.text());
    // Outside the household: no profile to pick after signing in.
    let (ann, ann_id) = app.user(&admin, "ann", json!({"home": false})).await;
    (app, idp, admin, ann, ann_id)
}

/// Asks to link the next sign-in to the user signed in with `token`, from
/// a page at `origin`.
async fn ask_to_link(app: &TestApp, token: &str, password: &str, origin: &str) -> Reply {
    let body = json!({"password": password});
    app.call(Method::POST, "/api/auth/sso/link", Some(token), Some(body), &[("host", HOST), ("origin", origin)]).await
}

/// Links the provider's `sub` to the user signed in with `token` (password
/// `test-pass-1`).
async fn link(app: &TestApp, idp: &Idp, token: &str, sub: &str) {
    let asked = ask_to_link(app, token, "test-pass-1", PUBLIC).await;
    assert_eq!(asked.status, StatusCode::NO_CONTENT, "{}", asked.text());
    let request = set_cookie(&asked, "pley_sso_link").unwrap();
    let started = start(app, idp, &format!("pley_token={token}; pley_sso_link={request}")).await;
    assert_eq!(started.get("prompt"), "login", "linking signs in at the provider anew");
    let code = format!("link-{sub}");
    started.grant(idp, &code, json!({"sub": sub, "email": format!("{sub}@example.org")}));
    let linked = back(app, &started, &code, &format!("pley_token={token}")).await;
    assert_eq!(location(&linked), "/account?sso=linked");
}

async fn link_ann(app: &TestApp, idp: &Idp, ann: &str) {
    link(app, idp, ann, "ann-at-the-provider").await;
}

#[tokio::test]
async fn people_sign_in_with_the_identity_linked_to_their_account() {
    let (app, idp, _admin, ann, ann_id) = world(false).await;
    let server = app.call(Method::GET, "/api/server", None, None, &[]).await;
    assert_eq!(server.body["sso"], json!({"name": "Stand-in", "start": format!("{PUBLIC}/api/auth/sso/start")}));

    // Nobody is linked yet: no account.
    let started = start(&app, &idp, "").await;
    assert_eq!(started.get("client_id"), "pley");
    assert_eq!(started.get("redirect_uri"), format!("{PUBLIC}/api/auth/sso/callback"));
    assert_eq!(started.get("code_challenge_method"), "S256");
    assert_eq!(started.get("scope"), "openid email", "who signed in, and nothing more");
    assert_eq!(started.get("prompt"), "");
    started.grant(&idp, "code-1", json!({}));
    assert_eq!(location(&back(&app, &started, "code-1", "").await), "/login?sso=unknown");

    link_ann(&app, &idp, &ann).await;
    let mine = app.get("/api/me/sso", &ann).await;
    assert_eq!(mine.body["linked"], true);
    assert_eq!(mine.body["email"], "ann-at-the-provider@example.org");
    let users = app.get("/api/users", &app.login("admin", "secret-one").await).await;
    let listed = users.body.as_array().unwrap().iter().find(|u| u["id"] == ann_id).unwrap().clone();
    assert_eq!((listed["sso"].as_bool(), listed["has_password"].as_bool()), (Some(true), Some(true)));

    // Now the identity signs in to ann's account, back on the page asked for.
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "code-2", json!({}));
    let signed_in = back(&app, &started, "code-2", "").await;
    assert_eq!(location(&signed_in), "/library/nckkvbd");
    assert_eq!(set_cookie(&signed_in, "pley_sso").as_deref(), Some(""), "the sign-in's cookie goes");
    let token = set_cookie(&signed_in, "pley_token").unwrap();
    let me = app.get("/api/auth/me", &token).await;
    assert_eq!(me.body["username"], "ann");

    // Each code works once.
    let again = back(&app, &started, "code-2", "").await;
    assert_eq!(location(&again), "/login?sso=provider");

    // Members of the household pick a profile first, as after a password.
    let (_cat, _) = app.user(&app.login("admin", "secret-one").await, "cat", json!({})).await;
    link(&app, &idp, &app.login("cat", "test-pass-1").await, "cat-at-the-provider").await;
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "cat", json!({"sub": "cat-at-the-provider"}));
    assert_eq!(location(&back(&app, &started, "cat", "").await), "/profiles?next=%2Flibrary%2Fnckkvbd");
}

#[tokio::test]
async fn sign_ins_only_finish_in_the_browser_that_started_them() {
    let (app, idp, _admin, ann, _) = world(false).await;
    link_ann(&app, &idp, &ann).await;
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "code", json!({}));
    // Someone else's browser, sent the link back from the provider: no cookie.
    let elsewhere = app
        .call(
            Method::GET,
            &format!("/api/auth/sso/callback?code=code&state={}", started.get("state")),
            None,
            None,
            &[("host", HOST)],
        )
        .await;
    assert_eq!(location(&elsewhere), "/login?sso=expired");
    assert_eq!(set_cookie(&elsewhere, "pley_token"), None);
    // Another sign-in's state, or a cookie made up.
    let other = start(&app, &idp, "").await;
    let mixed = Started { query: other.query.clone(), cookie: started.cookie.clone() };
    assert_eq!(location(&back(&app, &mixed, "code", "").await), "/login?sso=expired");
    let forged = Started { query: started.query.clone(), cookie: "AAAA".repeat(30) };
    assert_eq!(location(&back(&app, &forged, "code", "").await), "/login?sso=expired");
    // The provider said no.
    let denied = app
        .call(
            Method::GET,
            &format!("/api/auth/sso/callback?error=access_denied&state={}", started.get("state")),
            None,
            None,
            &with_cookies(&format!("pley_sso={}", started.cookie)),
        )
        .await;
    assert_eq!(location(&denied), "/login?sso=denied");
}

#[tokio::test]
async fn id_tokens_must_be_for_this_client_this_sign_in_and_now() {
    let (app, idp, _admin, ann, _) = world(false).await;
    link_ann(&app, &idp, &ann).await;
    let refused = [
        ("another nonce", json!({"nonce": "not-this-one"})),
        ("no nonce", json!({"nonce": null})),
        ("another client", json!({"aud": "someone-else"})),
        ("several audiences, issued to another", json!({"aud": ["pley", "other"], "azp": "other"})),
        ("another issuer", json!({"iss": "https://evil.example"})),
        ("expired", json!({"exp": now() - 600})),
        ("nobody", json!({"sub": ""})),
    ];
    for (n, (why, claims)) in refused.into_iter().enumerate() {
        let started = start(&app, &idp, "").await;
        started.grant(&idp, &format!("bad-{n}"), claims);
        assert_eq!(location(&back(&app, &started, &format!("bad-{n}"), "").await), "/login?sso=invalid", "{why}");
    }
    // Signed by a key the provider doesn't list (under its key's id, or another).
    for (n, kid) in ["k1", "k9"].into_iter().enumerate() {
        let started = start(&app, &idp, "").await;
        let code = format!("foreign-{n}");
        started.grant(&idp, &code, json!({}));
        idp.grants.lock().get_mut(&code).unwrap().key = Some((new_key(), kid.into()));
        assert_eq!(location(&back(&app, &started, &code, "").await), "/login?sso=invalid", "{kid}");
    }
    // A code redeemed with another verifier (PKCE): the provider refuses.
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "pkce", json!({}));
    idp.grants.lock().get_mut("pkce").unwrap().challenge = "something-else".into();
    assert_eq!(location(&back(&app, &started, "pkce", "").await), "/login?sso=provider");
    // HS256 with the client secret is fine for a registered client.
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "hs", json!({}));
    idp.grants.lock().get_mut("hs").unwrap().secret = Some("s3cret".into());
    assert_eq!(location(&back(&app, &started, "hs", "").await), "/library/nckkvbd");
}

#[tokio::test]
async fn new_accounts_start_without_libraries_and_only_when_allowed() {
    let (app, idp, admin, _ann, _) = world(false).await;
    app.post("/api/libraries", &admin, json!({"name": "Movies", "kind": "movies", "paths": [app.media_path("")]}))
        .await;
    let allowed = app.put("/api/settings", &admin, json!({"sso_new_accounts": true})).await;
    assert_eq!(allowed.status, StatusCode::OK);
    let started = start(&app, &idp, "").await;
    assert_eq!(started.get("scope"), "openid email profile", "a name for the new account");
    // The ID token says little; the userinfo endpoint the rest.
    *idp.userinfo.lock() =
        json!({"sub": "bea-at-the-provider", "preferred_username": "bea", "name": "Bea", "email": "bea@example.org"});
    started.grant(&idp, "new", json!({"sub": "bea-at-the-provider"}));
    let signed_in = back(&app, &started, "new", "").await;
    assert_eq!(location(&signed_in), "/library/nckkvbd");
    let token = set_cookie(&signed_in, "pley_token").unwrap();
    let me = app.get("/api/auth/me", &token).await.body;
    assert_eq!((me["username"].as_str(), me["name"].as_str()), (Some("bea"), Some("Bea")));
    assert_eq!((me["is_admin"].as_bool(), me["all_libraries"].as_bool()), (Some(false), Some(false)));
    assert_eq!(me["library_ids"], json!([]));
    assert_eq!((me["home"].as_bool(), me["has_password"].as_bool()), (Some(false), Some(false)));
    assert_eq!(
        app.get("/api/libraries", &token).await.body,
        json!([]),
        "nothing to see until an administrator says so"
    );
    // No password: it can't be changed, and the identity can't be unlinked.
    let change =
        app.put("/api/auth/password", &token, json!({"current_password": "", "new_password": "a-new-one-1"})).await;
    assert_eq!(change.status, StatusCode::BAD_REQUEST);
    let unlink = app.call(Method::DELETE, "/api/me/sso", Some(&token), Some(json!({})), &[]).await;
    assert_eq!(unlink.status, StatusCode::FORBIDDEN);
    // The same person again: the same account.
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "again", json!({"sub": "bea-at-the-provider"}));
    let token = set_cookie(&back(&app, &started, "again", "").await, "pley_token").unwrap();
    assert_eq!(app.get("/api/auth/me", &token).await.body["id"], me["id"]);
    // A disabled account stays out.
    let id = me["id"].as_i64().unwrap();
    app.patch(&format!("/api/users/{id}"), &admin, json!({"disabled": true})).await;
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "disabled", json!({"sub": "bea-at-the-provider"}));
    assert_eq!(location(&back(&app, &started, "disabled", "").await), "/login?sso=disabled");
}

#[tokio::test]
async fn only_the_group_that_may_sign_in_gets_in() {
    let (app, idp, admin, ann, _) = world(false).await;
    link_ann(&app, &idp, &ann).await;
    app.put("/api/settings", &admin, json!({"sso_group": "pley-users"})).await;
    let started = start(&app, &idp, "").await;
    assert_eq!(started.get("scope"), "openid email groups");
    started.grant(&idp, "outsider", json!({"groups": ["family"]}));
    assert_eq!(location(&back(&app, &started, "outsider", "").await), "/login?sso=group");
    // Groups from the ID token…
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "member", json!({"groups": ["family", "pley-users"]}));
    assert_eq!(location(&back(&app, &started, "member", "").await), "/library/nckkvbd");
    // …or, when it has none, from the userinfo endpoint (Keycloak's paths too).
    *idp.userinfo.lock() = json!({"sub": "ann-at-the-provider", "groups": ["/pley-users"]});
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "userinfo", json!({}));
    assert_eq!(location(&back(&app, &started, "userinfo", "").await), "/library/nckkvbd");
    // Userinfo about someone else is ignored.
    *idp.userinfo.lock() = json!({"sub": "someone-else", "groups": ["pley-users"]});
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "mixup", json!({}));
    assert_eq!(location(&back(&app, &started, "mixup", "").await), "/login?sso=group");
}

#[tokio::test]
async fn linking_takes_the_password_and_the_public_address() {
    let (app, idp, admin, ann, ann_id) = world(false).await;
    let wrong = ask_to_link(&app, &ann, "wrong", PUBLIC).await;
    assert_eq!(wrong.status, StatusCode::FORBIDDEN, "a session alone isn't enough");
    let elsewhere = ask_to_link(&app, &ann, "test-pass-1", "http://192.168.1.20:7539").await;
    assert_eq!(elsewhere.status, StatusCode::CONFLICT);
    assert!(elsewhere.text().contains(PUBLIC), "{}", elsewhere.text());

    // A link request only works for the user who asked.
    let asked = ask_to_link(&app, &ann, "test-pass-1", PUBLIC).await;
    let link = set_cookie(&asked, "pley_sso_link").unwrap();
    let (bob, _) = app.user(&admin, "bob", json!({})).await;
    let as_bob = app
        .call(
            Method::GET,
            "/api/auth/sso/start",
            None,
            None,
            &with_cookies(&format!("pley_token={bob}; pley_sso_link={link}")),
        )
        .await;
    assert_eq!(location(&as_bob), "/account?sso=signin");

    link_ann(&app, &idp, &ann).await;
    // Bob can't take ann's identity.
    let asked = ask_to_link(&app, &bob, "test-pass-1", PUBLIC).await;
    let link = set_cookie(&asked, "pley_sso_link").unwrap();
    let started = start(&app, &idp, &format!("pley_token={bob}; pley_sso_link={link}")).await;
    started.grant(&idp, "steal", json!({}));
    assert_eq!(location(&back(&app, &started, "steal", &format!("pley_token={bob}")).await), "/account?sso=taken");

    // Unlinking takes the password too; administrators can always unlink.
    let unlink = |password: &'static str| {
        app.call(Method::DELETE, "/api/me/sso", Some(&ann), Some(json!({"password": password})), &[])
    };
    assert_eq!(unlink("wrong").await.status, StatusCode::FORBIDDEN);
    assert_eq!(unlink("test-pass-1").await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/me/sso", &ann).await.body["linked"], false);
    link_ann(&app, &idp, &ann).await;
    assert_eq!(app.delete(&format!("/api/users/{ann_id}/sso"), &admin).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.delete(&format!("/api/users/{ann_id}/sso"), &ann).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn sign_ins_move_to_the_public_address() {
    let (app, _idp, _admin, _ann, _) = world(false).await;
    // From the address on the local network, to the public one, once.
    let local =
        app.call(Method::GET, "/api/auth/sso/start?next=/library", None, None, &[("host", "192.168.1.20:7539")]).await;
    assert_eq!(location(&local), format!("{PUBLIC}/api/auth/sso/start?next=%2Flibrary&hop=1"));
    assert_eq!(set_cookie(&local, "pley_sso"), None);
    // A proxy that rewrites Host doesn't make it go round in circles.
    let hopped = app
        .call(Method::GET, "/api/auth/sso/start?next=/library&hop=1", None, None, &[("host", "127.0.0.1:7539")])
        .await;
    assert!(location(&hopped).contains("/authorize?"));
    // Pages elsewhere aren't where people go afterwards.
    let away = app.call(Method::GET, "/api/auth/sso/start?next=//evil.example", None, None, &[("host", HOST)]).await;
    assert!(set_cookie(&away, "pley_sso").is_some());
}

#[tokio::test]
async fn providers_that_take_client_metadata_documents_need_no_registration() {
    let (app, idp, admin, ann, _) = world(true).await;
    let doc = app.call(Method::GET, "/api/auth/sso/client", None, None, &[]).await;
    assert_eq!(doc.status, StatusCode::OK);
    assert_eq!(doc.body["client_id"], format!("{PUBLIC}/api/auth/sso/client"));
    assert_eq!(doc.body["redirect_uris"], json!([format!("{PUBLIC}/api/auth/sso/callback")]));
    assert_eq!(
        (doc.body["token_endpoint_auth_method"].as_str(), doc.body["scope"].as_str()),
        (Some("none"), Some("openid email"))
    );
    link_ann(&app, &idp, &ann).await;
    let started = start(&app, &idp, "").await;
    assert_eq!(started.get("client_id"), format!("{PUBLIC}/api/auth/sso/client"));
    started.grant(&idp, "cimd", json!({}));
    assert_eq!(location(&back(&app, &started, "cimd", "").await), "/library/nckkvbd");
    // A public client has no secret to check an HS256 token with.
    let started = start(&app, &idp, "").await;
    started.grant(&idp, "hs", json!({}));
    idp.grants.lock().get_mut("hs").unwrap().secret = Some("guessable".into());
    assert_eq!(location(&back(&app, &started, "hs", "").await), "/login?sso=invalid");
    // With a client ID, there is no document.
    app.put("/api/settings", &admin, json!({"sso_client_id": "pley"})).await;
    assert_eq!(app.call(Method::GET, "/api/auth/sso/client", None, None, &[]).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_settings_test_the_provider_and_keep_the_secret() {
    let (app, idp, admin, ann, _) = world(false).await;
    let test = |body: Value| app.post("/api/settings/test", &admin, body);
    let found = test(json!({"service": "sso"})).await;
    assert_eq!(found.body["ok"], true, "{}", found.text());
    assert!(found.body["message"].as_str().unwrap().contains("/api/auth/sso/callback"), "{}", found.text());
    let cimd = test(json!({"service": "sso", "client_id": ""})).await;
    assert_eq!(cimd.body["ok"], false, "this provider doesn't take client metadata documents");
    let nowhere = test(json!({"service": "sso", "issuer": "http://127.0.0.1:9"})).await;
    assert_eq!(nowhere.body["ok"], false);
    let typo = test(json!({"service": "sso", "issuer": format!("{}/", idp.issuer)})).await;
    assert!(typo.body["message"].as_str().unwrap().contains("enter it exactly"), "{}", typo.text());
    assert_eq!(app.post("/api/settings/test", &ann, json!({"service": "sso"})).await.status, StatusCode::FORBIDDEN);

    let settings = app.get("/api/settings", &admin).await;
    assert_eq!(
        (settings.body["sso_client_secret"].as_str(), settings.body["sso_client_secret_set"].as_bool()),
        (Some(""), Some(true))
    );
    assert!(!settings.mentions("s3cret"));
    // Saving other settings keeps it.
    app.put("/api/settings", &admin, json!({"sso_name": "Home"})).await;
    assert_eq!(app.get("/api/settings", &admin).await.body["sso_client_secret_set"], true);
}
