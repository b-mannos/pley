//! Requests (`/api/requests`) and Discover (`/api/discover`): asking for
//! movies and shows the libraries don't have (see `crate::requests`).

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;

use super::truthy;
use crate::auth::{AdminUser, AuthUser};
use crate::discover::{List, MAX_PAGE, TitleKind};
use crate::error::{ApiResult, AppError};
use crate::requests::{
    Approval, DiscoverDetail, DiscoverPage, ListQuery, MediaRequest, NewRequest, RequestList, RequestResult,
    RequestStatus, Summary,
};
use crate::state::AppState;

/// What a search may be at most.
const QUERY_MAX: usize = 200;

/// `page` of a query: from 1 (the default) to TMDB's last.
fn page_of(query: &HashMap<String, String>) -> ApiResult<u32> {
    match query.get("page") {
        None => Ok(1),
        Some(page) => page
            .parse::<u32>()
            .ok()
            .filter(|p| (1..=MAX_PAGE).contains(p))
            .ok_or_else(|| AppError::bad_request(format!("page is 1 to {MAX_PAGE}"))),
    }
}

/// `GET /api/requests/summary`: whether this account may ask, and how much more.
pub async fn summary(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Summary>> {
    Ok(Json(state.requests.summary(&user).await?))
}

/// `GET /api/discover/{list}`.
pub async fn discover(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> RequestResult<Json<DiscoverPage>> {
    let list: List = serde_json::from_value(serde_json::Value::String(list))
        .map_err(|_| AppError::not_found("Discover lists trending, movies, shows, upcoming and anime"))?;
    let page = page_of(&query)?;
    Ok(Json(state.requests.discover(&user, list, page).await?))
}

/// `GET /api/discover/search?q=`.
pub async fn search(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<HashMap<String, String>>,
) -> RequestResult<Json<DiscoverPage>> {
    let text = query.get("q").map(|q| q.trim()).unwrap_or_default();
    if text.is_empty() || text.chars().count() > QUERY_MAX {
        return Err(AppError::bad_request(format!("search for 1 to {QUERY_MAX} characters")).into());
    }
    let page = page_of(&query)?;
    Ok(Json(state.requests.search(&user, text, page).await?))
}

/// `GET /api/discover/movie/{tmdb_id}`.
pub async fn movie(
    State(state): State<AppState>,
    user: AuthUser,
    Path(tmdb_id): Path<i64>,
) -> RequestResult<Json<DiscoverDetail>> {
    Ok(Json(state.requests.detail(&user, TitleKind::Movie, tmdb_id).await?))
}

/// `GET /api/discover/show/{tmdb_id}`.
pub async fn show(
    State(state): State<AppState>,
    user: AuthUser,
    Path(tmdb_id): Path<i64>,
) -> RequestResult<Json<DiscoverDetail>> {
    Ok(Json(state.requests.detail(&user, TitleKind::Show, tmdb_id).await?))
}

/// `GET /api/requests?status=&mine=&offset=&limit=`.
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<HashMap<String, String>>,
) -> RequestResult<Json<RequestList>> {
    let status = match query.get("status").map(String::as_str).filter(|s| !s.is_empty()) {
        Some(status) => Some(
            RequestStatus::parse(status)
                .ok_or_else(|| AppError::bad_request("status is pending, approved, declined, failed or available"))?,
        ),
        None => None,
    };
    let number = |name: &str| -> ApiResult<Option<i64>> {
        query
            .get(name)
            .map(|v| v.parse::<i64>().map_err(|_| AppError::bad_request(format!("{name} is a number"))))
            .transpose()
    };
    let list = ListQuery {
        status,
        mine: query.get("mine").is_some_and(|v| truthy(v)),
        offset: number("offset")?.unwrap_or(0),
        limit: number("limit")?,
    };
    Ok(Json(state.requests.list(&user, list).await?))
}

/// `POST /api/requests`: `{ kind, tmdb_id, seasons? }`.
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(new): Json<NewRequest>,
) -> RequestResult<(StatusCode, Json<MediaRequest>)> {
    Ok((StatusCode::CREATED, Json(state.requests.create(&user, new).await?)))
}

/// `GET /api/requests/{id}`.
pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> RequestResult<Json<MediaRequest>> {
    Ok(Json(state.requests.get(&user, id).await?))
}

/// `POST /api/requests/{id}/approve`: `{ quality_profile?, root_folder? }`.
pub async fn approve(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<i64>,
    body: Option<Json<Approval>>,
) -> RequestResult<Json<MediaRequest>> {
    let approval = body.map(|Json(a)| a).unwrap_or_default();
    Ok(Json(state.requests.approve(&admin, id, approval).await?))
}

#[derive(Debug, Default, Deserialize)]
pub struct Decline {
    #[serde(default)]
    reason: Option<String>,
}

/// `POST /api/requests/{id}/decline`: `{ reason? }`.
pub async fn decline(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<i64>,
    body: Option<Json<Decline>>,
) -> RequestResult<Json<MediaRequest>> {
    let reason = body.and_then(|Json(d)| d.reason);
    Ok(Json(state.requests.decline(&admin, id, reason).await?))
}

/// `DELETE /api/requests/{id}`.
pub async fn delete(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> RequestResult<StatusCode> {
    state.requests.delete(&user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
