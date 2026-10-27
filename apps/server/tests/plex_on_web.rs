//! The Plex-compatible API on pley's own port (`plex_bind = "web"`, the
//! default, src/plex/mod.rs `on_web_port`): its clients get it, browsers the
//! web app, on the same paths. The other Plex tests go through this port too.

mod common;

use axum::http::{Method, StatusCode, header};

use common::TestApp;

fn header(reply: &common::Reply, name: header::HeaderName) -> &str {
    reply.headers.get(name).and_then(|v| v.to_str().ok()).unwrap_or("")
}

#[tokio::test]
async fn its_clients_and_browsers_share_pleys_port() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;

    // A Plex client's `/`: the API, sandboxed on the web app's origin, kept
    // apart from the web app's `/` by caches.
    let root = app.plex(Method::GET, "/", &admin, &[]).await;
    assert_eq!(root.status, StatusCode::OK, "{}", root.text());
    assert!(root.body["MediaContainer"]["machineIdentifier"].is_string(), "{}", root.text());
    assert!(header(&root, header::CONTENT_SECURITY_POLICY).contains("sandbox"));
    let cache = header(&root, header::CACHE_CONTROL);
    assert!(!cache.is_empty() && !cache.contains("public"), "{cache}");
    assert!(header(&root, header::VARY).contains("X-Plex-Token"));

    // A browser's `/`: the web app, varying the same way.
    let page = app.call(Method::GET, "/", None, None, &[]).await;
    assert!(!page.text().contains("MediaContainer"), "{}", page.text());
    assert!(header(&page, header::VARY).contains("X-Plex-Token"));

    // Its own paths need no headers (a player's segments, /identity), and
    // the browser's cookie opens nothing there.
    let identity = app.call(Method::GET, "/identity", None, None, &[]).await;
    assert!(identity.text().contains("machineIdentifier"), "{}", identity.text());
    let cookie = format!("pley_token={admin}");
    let sections = app.call(Method::GET, "/library/sections", None, None, &[("cookie", &cookie)]).await;
    assert_eq!(sections.status, StatusCode::UNAUTHORIZED);

    // pley's own API stays pley's, whatever a request carries.
    let health = app.call(Method::GET, "/api/health", None, None, &[("x-plex-token", &admin)]).await;
    assert_eq!(health.body["status"], "ok");
}

#[tokio::test]
async fn off_it_answers_nowhere_on_pleys_port() {
    let app = TestApp::with_config(|config| config.plex_on_web = false).await;
    let identity = app.call(Method::GET, "/identity", None, None, &[]).await;
    assert!(!identity.text().contains("machineIdentifier"), "{}", identity.text());
    let admin = app.setup_admin().await;
    let root = app.call(Method::GET, "/", None, None, &[("x-plex-token", &admin)]).await;
    assert!(!root.text().contains("MediaContainer"), "{}", root.text());
}
