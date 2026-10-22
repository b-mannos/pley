use std::collections::HashMap;
use std::path::PathBuf;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use super::truthy;
use crate::auth::{AdminUser, AuthUser};
use crate::catalog::{self, ItemSummary, LibraryQuery, SortKey};
use crate::error::{ApiResult, AppError};
use crate::events::ServerEvent;
use crate::library::{LibraryKind, LibraryRow, get_library, list_libraries};
use crate::state::AppState;
use crate::util::now;

#[derive(Serialize)]
pub struct LibraryDto {
    id: i64,
    name: String,
    kind: LibraryKind,
    paths: Vec<String>,
    item_count: i64,
    created_at: i64,
    scanned_at: Option<i64>,
    scanning: bool,
}

async fn to_dto(state: &AppState, lib: &LibraryRow) -> ApiResult<LibraryDto> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE library_id = ? AND kind = ?")
        .bind(lib.id)
        .bind(lib.kind().top_level_kind())
        .fetch_one(&state.db.read)
        .await?;
    Ok(LibraryDto {
        id: lib.id,
        name: lib.name.clone(),
        kind: lib.kind(),
        paths: lib.paths().iter().map(|p| p.to_string_lossy().into_owned()).collect(),
        item_count: count,
        created_at: lib.created_at,
        scanned_at: lib.scanned_at,
        scanning: state.scanner.is_scanning(lib.id),
    })
}

async fn library_or_404(state: &AppState, id: i64) -> ApiResult<LibraryRow> {
    get_library(&state.db, id).await?.ok_or_else(|| AppError::not_found(format!("library {id} not found")))
}

pub async fn list(State(state): State<AppState>, _user: AuthUser) -> ApiResult<Json<Vec<LibraryDto>>> {
    let mut out = Vec::new();
    for lib in list_libraries(&state.db).await? {
        out.push(to_dto(&state, &lib).await?);
    }
    Ok(Json(out))
}

pub async fn get(State(state): State<AppState>, _user: AuthUser, Path(id): Path<i64>) -> ApiResult<Json<LibraryDto>> {
    let lib = library_or_404(&state, id).await?;
    Ok(Json(to_dto(&state, &lib).await?))
}

#[derive(Deserialize)]
pub struct CreateLibrary {
    name: String,
    kind: LibraryKind,
    paths: Vec<String>,
}

fn validate_paths(paths: &[String]) -> ApiResult<Vec<String>> {
    let mut out = Vec::new();
    for raw in paths {
        let p = raw.trim();
        if p.is_empty() {
            continue;
        }
        let path = PathBuf::from(p);
        if !path.is_absolute() {
            return Err(AppError::bad_request(format!("'{p}' is not an absolute path")));
        }
        if !path.is_dir() {
            return Err(AppError::bad_request(format!("folder '{p}' does not exist or is not accessible")));
        }
        let normalized = path.to_string_lossy().trim_end_matches(['/', '\\']).to_owned();
        let normalized = if normalized.ends_with(':') || normalized.is_empty() { p.to_owned() } else { normalized };
        if !out.contains(&normalized) {
            out.push(normalized);
        }
    }
    if out.is_empty() {
        return Err(AppError::bad_request("add at least one folder"));
    }
    Ok(out)
}

pub async fn create(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<CreateLibrary>,
) -> ApiResult<(StatusCode, Json<LibraryDto>)> {
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(AppError::bad_request("library name must be 1-100 characters"));
    }
    let paths = validate_paths(&body.paths)?;
    let id: i64 =
        sqlx::query_scalar("INSERT INTO libraries (name, kind, paths, created_at) VALUES (?, ?, ?, ?) RETURNING id")
            .bind(name)
            .bind(body.kind.as_str())
            .bind(serde_json::to_string(&paths).map_err(|e| AppError::Internal(e.into()))?)
            .bind(now())
            .fetch_one(&state.db.write)
            .await?;
    state.scanner.enqueue(id, false);
    state.watcher.reload().await;
    state.events.send(ServerEvent::LibrariesChanged);
    let lib = library_or_404(&state, id).await?;
    Ok((StatusCode::CREATED, Json(to_dto(&state, &lib).await?)))
}

#[derive(Deserialize)]
pub struct UpdateLibrary {
    name: Option<String>,
    paths: Option<Vec<String>>,
}

pub async fn update(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateLibrary>,
) -> ApiResult<Json<LibraryDto>> {
    let lib = library_or_404(&state, id).await?;
    if let Some(name) = body.name.as_deref().map(str::trim) {
        if name.is_empty() || name.chars().count() > 100 {
            return Err(AppError::bad_request("library name must be 1-100 characters"));
        }
        sqlx::query("UPDATE libraries SET name = ? WHERE id = ?").bind(name).bind(id).execute(&state.db.write).await?;
    }
    if let Some(paths) = &body.paths {
        let paths = validate_paths(paths)?;
        let json = serde_json::to_string(&paths).map_err(|e| AppError::Internal(e.into()))?;
        if json != lib.paths {
            sqlx::query("UPDATE libraries SET paths = ? WHERE id = ?")
                .bind(json)
                .bind(id)
                .execute(&state.db.write)
                .await?;
            state.scanner.enqueue(id, false);
            state.watcher.reload().await;
        }
    }
    state.events.send(ServerEvent::LibrariesChanged);
    let lib = library_or_404(&state, id).await?;
    Ok(Json(to_dto(&state, &lib).await?))
}

pub async fn remove(State(state): State<AppState>, _admin: AdminUser, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let result = sqlx::query("DELETE FROM libraries WHERE id = ?").bind(id).execute(&state.db.write).await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found(format!("library {id} not found")));
    }
    state.watcher.reload().await;
    state.events.send(ServerEvent::LibrariesChanged);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, Default)]
pub struct ScanBody {
    #[serde(default)]
    force: bool,
}

pub async fn scan(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    body: Option<Json<ScanBody>>,
) -> ApiResult<StatusCode> {
    library_or_404(&state, id).await?;
    state.scanner.enqueue(id, body.map(|b| b.force).unwrap_or(false));
    Ok(StatusCode::ACCEPTED)
}

pub async fn scan_all(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<StatusCode> {
    state.scanner.enqueue_all(false).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Serialize)]
pub struct ItemsPage {
    total: usize,
    items: Vec<ItemSummary>,
}

pub async fn items(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<ItemsPage>> {
    let lib = library_or_404(&state, id).await?;
    let sort = query
        .get("sort")
        .map(|s| SortKey::parse(s).ok_or_else(|| AppError::bad_request(format!("unknown sort '{s}'"))))
        .transpose()?;
    let sort = sort.unwrap_or(SortKey::Title);
    let descending = match query.get("order").map(String::as_str) {
        Some("desc") => true,
        Some("asc") => false,
        _ => matches!(sort, SortKey::Added | SortKey::Rating | SortKey::Release | SortKey::LastPlayed),
    };
    let year = query.get("year").and_then(|y| y.parse().ok());
    let q = LibraryQuery {
        library_id: lib.id,
        kind: lib.kind().top_level_kind(),
        sort,
        descending,
        genre: query.get("genre").map(String::as_str).filter(|g| !g.is_empty()),
        year,
    };
    let rows = catalog::library_items(&state.db, user.id, &q).await?;
    let mut items: Vec<ItemSummary> = rows.iter().map(|r| r.to_summary()).collect();
    if let Some(watched) = query.get("watched") {
        let want = truthy(watched);
        items.retain(|i| i.user_state.watched == want);
    }
    let total = items.len();
    let offset = query.get("offset").and_then(|o| o.parse::<usize>().ok()).unwrap_or(0);
    let limit = query.get("limit").and_then(|l| l.parse::<usize>().ok()).unwrap_or(usize::MAX);
    let items = items.into_iter().skip(offset).take(limit).collect();
    Ok(Json(ItemsPage { total, items }))
}

pub async fn filters(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let lib = library_or_404(&state, id).await?;
    let (genres, years) = catalog::library_filters(&state.db, lib.id, lib.kind().top_level_kind()).await?;
    Ok(Json(serde_json::json!({ "genres": genres, "years": years })))
}

#[derive(Serialize)]
pub struct Hub {
    pub key: String,
    pub title: String,
    pub library_id: Option<i64>,
    pub items: Vec<ItemSummary>,
}

#[derive(Serialize)]
pub struct Hubs {
    pub hubs: Vec<Hub>,
}

pub async fn hubs(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<Json<Hubs>> {
    let lib = library_or_404(&state, id).await?;
    let kind = lib.kind();
    let mut hubs = Vec::new();
    let summarize = |rows: Vec<catalog::ItemRow>| rows.iter().map(|r| r.to_summary()).collect::<Vec<_>>();
    if kind != LibraryKind::Music {
        let rows = catalog::continue_watching(&state.db, user.id, Some(lib.id), 20).await?;
        hubs.push(Hub {
            key: format!("continue_watching:{id}"),
            title: "Continue Watching".into(),
            library_id: Some(id),
            items: summarize(rows),
        });
    }
    let rows = catalog::recently_added(&state.db, user.id, lib.id, kind.as_str(), 30).await?;
    let title = match kind {
        LibraryKind::Music => "Recently Added Albums",
        LibraryKind::Shows => "Recently Added Shows",
        LibraryKind::Movies => "Recently Added",
    };
    hubs.push(Hub {
        key: format!("recently_added:{id}"),
        title: title.into(),
        library_id: Some(id),
        items: summarize(rows),
    });
    if kind != LibraryKind::Music {
        let top = kind.top_level_kind();
        let rows = catalog::recently_released(&state.db, user.id, lib.id, top, 30).await?;
        hubs.push(Hub {
            key: format!("recently_released:{id}"),
            title: "Recently Released".into(),
            library_id: Some(id),
            items: summarize(rows),
        });
        let rows = catalog::top_rated(&state.db, user.id, lib.id, top, 30).await?;
        hubs.push(Hub {
            key: format!("top_rated:{id}"),
            title: "Top Rated".into(),
            library_id: Some(id),
            items: summarize(rows),
        });
    }
    hubs.retain(|h| !h.items.is_empty());
    Ok(Json(Hubs { hubs }))
}
