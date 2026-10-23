//! An in-process pley for the integration tests that use it (tests/api.rs
//! and friends have their own copies): a temporary data directory, the pley
//! API and the Plex-compatible API on the same state, fake media files.
//! No transcoder runs unless a test points the config at one.

#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use pley::{AppState, Config};

pub struct TestApp {
    pub api: Router,
    pub plex: pley::plex::Service,
    pub state: AppState,
    pub media: tempfile::TempDir,
    _data: tempfile::TempDir,
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
    pub bytes: Vec<u8>,
}

impl Reply {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    /// Whether the raw response mentions `needle` anywhere (leak checks).
    pub fn mentions(&self, needle: &str) -> bool {
        self.text().contains(needle)
    }
}

async fn reply(resp: axum::response::Response) -> Reply {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024).await.unwrap().to_vec();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Reply { status, headers, body, bytes }
}

impl TestApp {
    pub async fn new() -> Self {
        Self::with_config(|_| {}).await
    }

    /// A pley whose configuration `adjust` may change (a transcoder URL, …).
    pub async fn with_config(adjust: impl FnOnce(&mut Config)) -> Self {
        let data = tempfile::tempdir().unwrap();
        let mut config = Config::for_data_dir(data.path());
        config.transcoder_path = Some(data.path().join("no-transcoder-in-tests"));
        adjust(&mut config);
        let state = AppState::init(config).await.unwrap();
        let api = pley::api::router(state.clone());
        let plex = pley::plex::service(state.clone());
        Self { api, plex, state, media: tempfile::tempdir().unwrap(), _data: data }
    }

    pub fn media_file(&self, rel: &str, bytes: usize) {
        let path = self.media.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7u8; bytes]).unwrap();
    }

    pub fn media_path(&self, rel: &str) -> String {
        self.media.path().join(rel).to_string_lossy().into_owned()
    }

    pub async fn call(
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
        reply(self.api.clone().oneshot(req).await.unwrap()).await
    }

    pub async fn get(&self, uri: &str, token: &str) -> Reply {
        self.call(Method::GET, uri, Some(token), None, &[]).await
    }

    pub async fn post(&self, uri: &str, token: &str, body: Value) -> Reply {
        self.call(Method::POST, uri, Some(token), Some(body), &[]).await
    }

    pub async fn put(&self, uri: &str, token: &str, body: Value) -> Reply {
        self.call(Method::PUT, uri, Some(token), Some(body), &[]).await
    }

    pub async fn patch(&self, uri: &str, token: &str, body: Value) -> Reply {
        self.call(Method::PATCH, uri, Some(token), Some(body), &[]).await
    }

    pub async fn delete(&self, uri: &str, token: &str) -> Reply {
        self.call(Method::DELETE, uri, Some(token), None, &[]).await
    }

    /// The Plex-compatible API, asking for JSON with the token as Plex
    /// clients send it.
    pub async fn plex(&self, method: Method, uri: &str, token: &str, extra: &[(&str, &str)]) -> Reply {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("x-plex-token", token)
            .header(header::ACCEPT, "application/json");
        for (k, v) in extra {
            req = req.header(*k, *v);
        }
        reply(self.plex.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap()).await
    }

    /// A Plex-compatible API request with `body` (an upload).
    pub async fn plex_upload(&self, method: Method, uri: &str, token: &str, body: Vec<u8>) -> Reply {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("x-plex-token", token)
            .header(header::ACCEPT, "application/json")
            .body(Body::from(body))
            .unwrap();
        reply(self.plex.clone().oneshot(req).await.unwrap()).await
    }

    /// `MediaContainer` of a successful Plex request.
    pub async fn plex_json(&self, uri: &str, token: &str) -> Value {
        let reply = self.plex(Method::GET, uri, token, &[]).await;
        assert_eq!(reply.status, StatusCode::OK, "{uri}: {}", reply.text());
        reply.body["MediaContainer"].clone()
    }

    pub async fn setup_admin(&self) -> String {
        let r = self
            .call(Method::POST, "/api/setup", None, Some(json!({"username": "admin", "password": "secret1"})), &[])
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        r.body["token"].as_str().unwrap().to_owned()
    }

    pub async fn try_login(&self, username: &str, password: &str) -> Reply {
        self.call(Method::POST, "/api/auth/login", None, Some(json!({"username": username, "password": password})), &[])
            .await
    }

    pub async fn login(&self, username: &str, password: &str) -> String {
        let r = self.try_login(username, password).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        r.body["token"].as_str().unwrap().to_owned()
    }

    /// A user with the password `password1` and the given fields, signed in:
    /// `(token, id)`.
    pub async fn user(&self, admin: &str, username: &str, mut fields: Value) -> (String, i64) {
        fields["username"] = json!(username);
        fields["password"] = json!("password1");
        let created = self.post("/api/users", admin, fields).await;
        assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
        (self.login(username, "password1").await, created.body["id"].as_i64().unwrap())
    }

    pub async fn wait_for_scans(&self) {
        for _ in 0..400 {
            if self.state.scanner.statuses().is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("scan did not finish");
    }

    /// A library of the media folder `rel`, scanned.
    pub async fn library(&self, token: &str, name: &str, kind: &str, rel: &str) -> i64 {
        std::fs::create_dir_all(self.media.path().join(rel)).unwrap();
        let r = self
            .post("/api/libraries", token, json!({"name": name, "kind": kind, "paths": [self.media_path(rel)]}))
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        self.wait_for_scans().await;
        r.body["id"].as_i64().unwrap()
    }

    /// Title → id of the top-level items of a library.
    pub async fn ids(&self, library: i64, token: &str) -> HashMap<String, i64> {
        let page = self.get(&format!("/api/libraries/{library}/items"), token).await;
        assert_eq!(page.status, StatusCode::OK, "{:?}", page.body);
        page.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| (i["title"].as_str().unwrap().to_owned(), i["id"].as_i64().unwrap()))
            .collect()
    }

    /// Ids of the children (seasons, episodes, albums, tracks) of an item, by title.
    pub async fn children(&self, id: i64, token: &str) -> HashMap<String, i64> {
        let page = self.get(&format!("/api/items/{id}/children"), token).await;
        assert_eq!(page.status, StatusCode::OK, "{:?}", page.body);
        page.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| (i["title"].as_str().unwrap().to_owned(), i["id"].as_i64().unwrap()))
            .collect()
    }

    /// What the metadata agent stores for a content rating.
    pub async fn set_rating(&self, title: &str, rating: Option<&str>) {
        sqlx::query("UPDATE items SET content_rating = ?, rating_age = ? WHERE title = ?")
            .bind(rating)
            .bind(rating.and_then(pley::ratings::rating_age))
            .bind(title)
            .execute(&self.state.db.write)
            .await
            .unwrap();
    }

    /// Id of the first media file of an item.
    pub async fn file_of(&self, item: i64) -> i64 {
        sqlx::query_scalar("SELECT id FROM media_files WHERE item_id = ? ORDER BY id")
            .bind(item)
            .fetch_one(&self.state.db.read)
            .await
            .unwrap()
    }
}

pub fn titles(items: &Value) -> Vec<String> {
    items.as_array().map(|a| a.iter().map(|i| i["title"].as_str().unwrap().to_owned()).collect()).unwrap_or_default()
}

/// Titles of the `Metadata` of a Plex `MediaContainer`.
pub fn plex_titles(container: &Value) -> Vec<String> {
    titles(&container["Metadata"])
}

/// Percent-encodes a query value.
pub fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
