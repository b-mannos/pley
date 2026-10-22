//! Serves the web UI. Release builds embed `apps/web/dist`; debug builds read it
//! from disk so a rebuilt UI shows up without recompiling. Precompressed
//! `.br`/`.gz` variants written by the Vite build are served when accepted.

use std::path::{Component, Path, PathBuf};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use crate::error::AppError;
use crate::state::AppState;

#[derive(RustEmbed)]
#[folder = "../web/dist/"]
#[allow_missing = true]
struct Assets;

struct Asset {
    bytes: Vec<u8>,
    etag: String,
}

fn embedded(path: &str) -> Option<Asset> {
    Assets::get(path).map(|f| Asset {
        etag: format!("\"{}\"", hex::encode(&f.metadata.sha256_hash()[..12])),
        bytes: f.data.into_owned(),
    })
}

fn from_dir(dir: &Path, path: &str) -> Option<Asset> {
    let rel = Path::new(path);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let full: PathBuf = dir.join(rel);
    let bytes = std::fs::read(&full).ok()?;
    let modified =
        std::fs::metadata(&full).and_then(|m| m.modified()).ok().map(crate::util::system_time_secs).unwrap_or(0);
    Some(Asset { etag: format!("\"{}-{modified}\"", bytes.len()), bytes })
}

pub async fn static_handler(State(state): State<AppState>, request: Request) -> Response {
    let path = request.uri().path().trim_start_matches('/').to_owned();
    if path.starts_with("api/") || path == "api" {
        return AppError::not_found("no such endpoint").into_response();
    }
    let load = |p: &str| match &state.config.web_dir {
        Some(dir) => from_dir(dir, p),
        None => embedded(p),
    };
    let accept =
        request.headers().get(header::ACCEPT_ENCODING).and_then(|v| v.to_str().ok()).unwrap_or_default().to_owned();

    let wanted = if path.is_empty() { "index.html".to_owned() } else { path };
    let (name, asset) = match load(&wanted) {
        Some(asset) => (wanted, asset),
        None => {
            // Client-side routes fall back to the SPA shell; missing assets 404.
            let looks_like_file = Path::new(&wanted).extension().is_some();
            match (looks_like_file, load("index.html")) {
                (false, Some(asset)) => ("index.html".to_owned(), asset),
                _ => {
                    return (StatusCode::NOT_FOUND, "web UI not built: run `npm run build` in web/").into_response();
                }
            }
        }
    };

    if request.headers().get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(asset.etag.as_str()) {
        return StatusCode::NOT_MODIFIED.into_response();
    }

    let mut headers = HeaderMap::new();
    let mime = mime_guess::from_path(&name).first_or_octet_stream();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref()).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    let cache = if name.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    if let Ok(etag) = HeaderValue::from_str(&asset.etag) {
        headers.insert(header::ETAG, etag);
    }
    headers.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));

    for (encoding, suffix) in [("br", ".br"), ("gzip", ".gz")] {
        if accept.split(',').any(|e| e.trim().split(';').next() == Some(encoding)) {
            if let Some(compressed) = load(&format!("{name}{suffix}")) {
                headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static(encoding));
                return (headers, Body::from(compressed.bytes)).into_response();
            }
        }
    }
    (headers, Body::from(asset.bytes)).into_response()
}
