//! "More Like This": movies or shows of the same library that share
//! collections (sequels), directors, creators and top-billed cast, genres,
//! and a period with an item. Scored in one query; access rules apply.

use crate::catalog::{self, ItemRow};
use crate::db::Db;

/// Candidates scored before access rules and the limit are applied.
const CANDIDATES: i64 = 120;

/// Items like `item_id`, best first.
pub async fn related(db: &Db, user_id: i64, item_id: i64, limit: usize) -> sqlx::Result<Vec<ItemRow>> {
    let ids: Vec<i64> = sqlx::query_scalar(
        "WITH target AS (SELECT id, library_id, kind, year, genres FROM items WHERE id = ?1),
         genres AS (
             SELECT i.id, COUNT(*) * 2.0 AS score
             FROM items i, json_each(i.genres) g, target t
             WHERE i.library_id = t.library_id AND i.kind = t.kind AND i.id != t.id
               AND g.value IN (SELECT value FROM json_each(t.genres))
             GROUP BY i.id),
         people AS (
             SELECT b.item_id AS id,
                    SUM(CASE WHEN a.role IN ('director', 'creator') THEN 3.0 ELSE 1.5 END) AS score
             FROM item_people a
             JOIN item_people b ON b.person_id = a.person_id AND b.role = a.role AND b.item_id != a.item_id
             WHERE a.item_id = ?1 AND a.role IN ('actor', 'director', 'creator')
               AND (a.role != 'actor' OR a.position < 8) AND (b.role != 'actor' OR b.position < 12)
             GROUP BY b.item_id),
         collections AS (
             SELECT DISTINCT b.item_id AS id, 6.0 AS score
             FROM collection_items a JOIN collection_items b ON b.collection_id = a.collection_id
             WHERE a.item_id = ?1 AND b.item_id != ?1),
         scores AS (
             SELECT id, SUM(score) AS score
             FROM (SELECT * FROM genres UNION ALL SELECT * FROM people UNION ALL SELECT * FROM collections)
             GROUP BY id)
         SELECT s.id FROM scores s JOIN items i ON i.id = s.id, target t
         WHERE i.library_id = t.library_id AND i.kind = t.kind
         ORDER BY s.score + CASE WHEN i.year IS NULL OR t.year IS NULL THEN 0
                                 ELSE MAX(0.0, 2.0 - ABS(i.year - t.year) / 5.0) END DESC,
                  COALESCE(i.rating, 0) DESC, i.sort_title
         LIMIT ?2",
    )
    .bind(item_id)
    .bind(CANDIDATES)
    .fetch_all(&db.read)
    .await?;
    let mut rows = catalog::items_by_ids(db, user_id, &ids).await?;
    rows.truncate(limit);
    Ok(rows)
}
