//! Hand-edited metadata. Each edited field is stored as an override and
//! written back onto the item after every metadata refresh and scan, so the
//! metadata agent never undoes an edit. Removing an override lets the next
//! refresh fill the field again.

use std::collections::HashMap;

use crate::db::Db;
use crate::error::AppError;
use crate::util::{now, sort_title};

/// Editable fields and their item columns.
pub const FIELDS: &[&str] = &[
    "title",
    "sort_title",
    "original_title",
    "year",
    "summary",
    "tagline",
    "studio",
    "content_rating",
    "release_date",
    "genres",
    "poster",
    "backdrop",
    "thumb",
];

pub fn validate(field: &str, value: Option<&str>) -> Result<(), AppError> {
    if !FIELDS.contains(&field) {
        return Err(AppError::bad_request(format!("'{field}' can't be edited")));
    }
    let Some(value) = value else { return Ok(()) };
    match field {
        "title" if value.trim().is_empty() => Err(AppError::bad_request("the title can't be empty")),
        "year" if value.parse::<i64>().map_or(true, |y| !(1800..=2200).contains(&y)) => {
            Err(AppError::bad_request("the year must be a number like 1999"))
        }
        "release_date" if !looks_like_date(value) => Err(AppError::bad_request("dates look like 1999-03-31")),
        "genres" if serde_json::from_str::<Vec<String>>(value).is_err() => {
            Err(AppError::bad_request("genres must be a JSON list of names"))
        }
        _ => Ok(()),
    }
}

fn looks_like_date(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && value.split('-').all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

/// Fields of an item that are overridden.
pub async fn locked_fields(db: &Db, item_id: i64) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT field FROM item_overrides WHERE item_id = ? ORDER BY field")
        .bind(item_id)
        .fetch_all(&db.read)
        .await
}

/// Store (`Some`) or drop (`None`) overrides, then apply them.
pub async fn set(db: &Db, item_id: i64, changes: &[(String, Option<String>)]) -> sqlx::Result<()> {
    let mut tx = db.write.begin().await?;
    for (field, value) in changes {
        match value {
            Some(value) => {
                sqlx::query(
                    "INSERT INTO item_overrides (item_id, field, value) VALUES (?, ?, ?)
                     ON CONFLICT(item_id, field) DO UPDATE SET value = excluded.value",
                )
                .bind(item_id)
                .bind(field)
                .bind(value)
                .execute(&mut *tx)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM item_overrides WHERE item_id = ? AND field = ?")
                    .bind(item_id)
                    .bind(field)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }
    tx.commit().await?;
    apply(db, &[item_id]).await
}

/// Write the overrides of `item_ids` onto the items.
pub async fn apply(db: &Db, item_ids: &[i64]) -> sqlx::Result<()> {
    if item_ids.is_empty() {
        return Ok(());
    }
    let placeholders = vec!["?"; item_ids.len()].join(",");
    let sql = format!("SELECT item_id, field, value FROM item_overrides WHERE item_id IN ({placeholders})");
    let mut query = sqlx::query_as::<_, (i64, String, Option<String>)>(&sql);
    for id in item_ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(&db.read).await?;
    let mut by_item: HashMap<i64, Vec<(String, Option<String>)>> = HashMap::new();
    for (item, field, value) in rows {
        by_item.entry(item).or_default().push((field, value));
    }
    if by_item.is_empty() {
        return Ok(());
    }
    let mut tx = db.write.begin().await?;
    for (item, fields) in by_item {
        let has_sort = fields.iter().any(|(f, _)| f == "sort_title");
        for (field, value) in &fields {
            if !FIELDS.contains(&field.as_str()) {
                continue;
            }
            // `field` is one of FIELDS, so formatting it into SQL is safe.
            sqlx::query(&format!("UPDATE items SET {field} = ? WHERE id = ?"))
                .bind(value)
                .bind(item)
                .execute(&mut *tx)
                .await?;
            match field.as_str() {
                "title" if !has_sort => {
                    sqlx::query("UPDATE items SET sort_title = ? WHERE id = ?")
                        .bind(value.as_deref().map(sort_title))
                        .bind(item)
                        .execute(&mut *tx)
                        .await?;
                }
                "content_rating" => {
                    sqlx::query("UPDATE items SET rating_age = ? WHERE id = ?")
                        .bind(value.as_deref().and_then(crate::ratings::rating_age))
                        .bind(item)
                        .execute(&mut *tx)
                        .await?;
                }
                _ => {}
            }
        }
        sqlx::query("UPDATE items SET updated_at = ? WHERE id = ?").bind(now()).bind(item).execute(&mut *tx).await?;
    }
    tx.commit().await
}

/// SQL for `UPDATE items …`: whether the row's content rating was set by
/// hand. Metadata refreshes leave it (and its age) alone instead of
/// overwriting it until the hand edit is re-applied: age limits depend on it.
pub const RATING_LOCKED: &str =
    "EXISTS (SELECT 1 FROM item_overrides o WHERE o.item_id = items.id AND o.field = 'content_rating')";

/// Re-apply overrides of every item in a library (after a scan).
pub async fn apply_library(db: &Db, library_id: i64) -> sqlx::Result<()> {
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT o.item_id FROM item_overrides o JOIN items i ON i.id = o.item_id WHERE i.library_id = ?",
    )
    .bind(library_id)
    .fetch_all(&db.read)
    .await?;
    apply(db, &ids).await
}

/// Re-apply overrides of an item and everything below it (after a refresh).
pub async fn apply_tree(db: &Db, item_id: i64) -> sqlx::Result<()> {
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT o.item_id FROM item_overrides o JOIN items i ON i.id = o.item_id
         WHERE i.id = ? OR i.parent_id = ? OR i.grandparent_id = ?",
    )
    .bind(item_id)
    .bind(item_id)
    .bind(item_id)
    .fetch_all(&db.read)
    .await?;
    apply(db, &ids).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        assert!(validate("title", Some("Alien")).is_ok());
        assert!(validate("title", Some("  ")).is_err());
        assert!(validate("year", Some("1979")).is_ok());
        assert!(validate("year", Some("79a")).is_err());
        assert!(validate("release_date", Some("1979-05-25")).is_ok());
        assert!(validate("release_date", Some("25/05/1979")).is_err());
        assert!(validate("genres", Some(r#"["Horror","Sci-Fi"]"#)).is_ok());
        assert!(validate("genres", Some("Horror")).is_err());
        assert!(validate("path", Some("/etc/passwd")).is_err());
        assert!(validate("summary", None).is_ok());
    }
}
