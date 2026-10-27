//! Settings → Network (src/network.rs): where pley listens, which an
//! administrator changes and pley saves in its config file. Listening anew
//! while pley runs is in lifecycle.rs.

mod common;

use axum::http::StatusCode;
use serde_json::json;

use common::TestApp;

#[tokio::test]
async fn administrators_see_where_pley_listens_and_could() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let reply = app.get("/api/network", &admin).await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    assert_eq!(reply.body["web"], json!(["127.0.0.1:0"]));
    assert_eq!(reply.body["plex"], json!([]));
    assert_eq!(reply.body["fixed"], json!({"web": false, "plex": false, "discovery": false}));
    assert_eq!(reply.body["windows"], cfg!(windows));
    assert!(reply.body["adapters"].is_array());
    assert!(reply.body["config_file"].as_str().unwrap().ends_with("pley.toml"));

    let (user, _) = app.user(&admin, "viewer", json!({})).await;
    assert_eq!(app.get("/api/network", &user).await.status, StatusCode::FORBIDDEN);
    let change = json!({"web": ["0.0.0.0:7539"], "plex": [], "discovery": true});
    assert_eq!(app.put("/api/network", &user, change).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_change_is_saved_for_the_next_start() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let change = json!({"web": ["127.0.0.1:7539", "192.168.1.10:7539"], "plex": [], "discovery": false});
    let reply = app.put("/api/network", &admin, change).await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    assert_eq!(reply.body["web"], json!(["127.0.0.1:7539", "192.168.1.10:7539"]));

    let saved = std::fs::read_to_string(&app.state.config.config_file).unwrap();
    assert!(saved.contains("bind = \"127.0.0.1:7539, 192.168.1.10:7539\"\n"), "{saved}");
    assert!(saved.contains("plex_bind = \"off\"\n"), "{saved}");
    assert!(saved.contains("discovery = false\n"), "{saved}");
    assert_eq!(app.get("/api/network", &admin).await.body["web"], json!(["127.0.0.1:7539", "192.168.1.10:7539"]));
}

#[tokio::test]
async fn what_pley_cant_listen_on_is_refused() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    for (change, why) in [
        (json!({"web": [], "plex": [], "discovery": true}), "nowhere"),
        (json!({"web": ["127.0.0.1:0"], "plex": [], "discovery": true}), "no port"),
        (json!({"web": ["0.0.0.0:7539", "192.168.1.10:7539"], "plex": [], "discovery": true}), "0.0.0.0 has them all"),
        (json!({"web": ["127.0.0.1:7539"], "plex": ["127.0.0.1:7539"], "discovery": true}), "one port, two APIs"),
        (json!({"web": ["somewhere"], "plex": [], "discovery": true}), "not an address"),
    ] {
        let reply = app.put("/api/network", &admin, change).await;
        assert!(reply.status.is_client_error(), "{why}: {:?}", reply.body);
    }
    assert!(!app.state.config.config_file.exists(), "nothing was saved");
}

#[tokio::test]
async fn what_the_command_line_sets_stays() {
    // As with --bind 127.0.0.1:7539 (this app listens nowhere).
    let app = TestApp::with_config(|config| {
        config.bind = "127.0.0.1:7539".parse().unwrap();
        config.network_fixed.web = true;
    })
    .await;
    let admin = app.setup_admin().await;
    assert_eq!(app.get("/api/network", &admin).await.body["fixed"]["web"], true);
    let elsewhere = json!({"web": ["0.0.0.0:7539"], "plex": [], "discovery": false});
    let reply = app.put("/api/network", &admin, elsewhere).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{:?}", reply.body);

    // The rest can change, and is saved without the address.
    let plex = json!({"web": ["127.0.0.1:7539"], "plex": ["127.0.0.1:32400"], "discovery": false});
    let reply = app.put("/api/network", &admin, plex).await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    let saved = std::fs::read_to_string(&app.state.config.config_file).unwrap();
    assert!(!saved.lines().any(|line| line.starts_with("bind")), "{saved}");
    assert!(saved.contains("plex_bind = \"127.0.0.1:32400\""), "{saved}");
}
