//! Who may see what.
//!
//! Administrators see everything. Other users see all libraries or a chosen
//! set and, with an age limit, only movies and shows rated for that age:
//! music is exempt, seasons and episodes go by their show's rating (or their
//! own, when that is stricter) and unrated titles are hidden. Item queries apply the same rules in SQL (see
//! `catalog::VISIBLE`); this module answers them for whole libraries.

use crate::auth::AuthUser;
use crate::db::Db;
use crate::library::{LibraryRow, list_libraries};

/// Library ids the user may open; `None` means all of them.
pub async fn library_ids(db: &Db, user: &AuthUser) -> sqlx::Result<Option<Vec<i64>>> {
    if user.is_admin || user.all_libraries {
        return Ok(None);
    }
    let ids = sqlx::query_scalar("SELECT library_id FROM user_libraries WHERE user_id = ?")
        .bind(user.id)
        .fetch_all(&db.read)
        .await?;
    Ok(Some(ids))
}

pub async fn visible_libraries(db: &Db, user: &AuthUser) -> sqlx::Result<Vec<LibraryRow>> {
    let all = list_libraries(db).await?;
    Ok(match library_ids(db, user).await? {
        None => all,
        Some(ids) => all.into_iter().filter(|l| ids.contains(&l.id)).collect(),
    })
}

pub async fn can_open_library(db: &Db, user: &AuthUser, library_id: i64) -> sqlx::Result<bool> {
    Ok(library_ids(db, user).await?.is_none_or(|ids| ids.contains(&library_id)))
}
