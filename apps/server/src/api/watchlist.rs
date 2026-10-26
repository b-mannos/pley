//! My List (`/api/watchlist`): movies and shows a user keeps to watch later.
//! A movie leaves it once watched (`catalog::record_progress`,
//! `catalog::set_watched`); a show stays until it is removed.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Serialize;

use super::items::item_or_404;
use crate::auth::AuthUser;
use crate::catalog::{self, ItemRow, ItemSummary};
use crate::error::{ApiResult, AppError};
use crate::events::ServerEvent;
use crate::state::AppState;

#[derive(Serialize)]
pub struct Watchlist {
    items: Vec<ItemSummary>,
}

/// `GET /api/watchlist`: the user's My List, newest added first.
pub async fn list(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Watchlist>> {
    let rows = catalog::watchlist(&state.db, user.id, None).await?;
    Ok(Json(Watchlist { items: rows.iter().map(ItemRow::to_summary).collect() }))
}

/// `PUT /api/watchlist/{id}`: adds a movie or a show (again: nothing changes).
pub async fn add(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let row = item_or_404(&state, user.id, id).await?;
    if !matches!(row.kind.as_str(), "movie" | "show") {
        return Err(AppError::bad_request("only movies and shows go in My List"));
    }
    if catalog::add_to_watchlist(&state.db, user.id, id).await? {
        state.events.send(ServerEvent::WatchlistChanged { user_id: user.id });
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/watchlist/{id}`: takes it out (not there: nothing changes),
/// also one the user may no longer see.
pub async fn remove(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    if catalog::remove_from_watchlist(&state.db, user.id, id).await? {
        state.events.send(ServerEvent::WatchlistChanged { user_id: user.id });
    } else {
        item_or_404(&state, user.id, id).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}
