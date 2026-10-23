use std::collections::HashMap;

use axum::Json;
use axum::extract::{Query, State};
use serde::Serialize;

use super::libraries::{Hub, Hubs};
use crate::auth::AuthUser;
use crate::catalog::{self, ItemSummary};
use crate::error::ApiResult;
use crate::library::list_libraries;
use crate::people::{self, PersonSummary};
use crate::state::AppState;

pub async fn home(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Hubs>> {
    let mut hubs = Vec::new();
    let continue_rows = catalog::continue_watching(&state.db, user.id, None, 20).await?;
    hubs.push(Hub {
        key: "continue_watching".into(),
        title: "Continue Watching".into(),
        library_id: None,
        items: continue_rows.iter().map(|r| r.to_summary()).collect(),
    });
    for lib in list_libraries(&state.db).await? {
        let rows = catalog::recently_added(&state.db, user.id, lib.id, lib.kind().as_str(), 20).await?;
        hubs.push(Hub {
            key: format!("recently_added:{}", lib.id),
            // "in" reads right whatever the library is called.
            title: format!("Recently Added in {}", lib.name),
            library_id: Some(lib.id),
            items: rows.iter().map(|r| r.to_summary()).collect(),
        });
    }
    hubs.retain(|h| !h.items.is_empty());
    Ok(Json(Hubs { hubs }))
}

#[derive(Serialize)]
pub struct SearchResults {
    items: Vec<ItemSummary>,
    people: Vec<PersonSummary>,
}

pub async fn search(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<SearchResults>> {
    let q = query.get("q").map(|s| s.trim()).unwrap_or_default();
    let limit = query.get("limit").and_then(|l| l.parse::<i64>().ok()).unwrap_or(40).clamp(1, 200);
    let rows = catalog::search(&state.db, user.id, q, limit).await?;
    let people = people::search(&state.db, user.id, q, 12).await?;
    Ok(Json(SearchResults { items: rows.iter().map(|r| r.to_summary()).collect(), people }))
}
