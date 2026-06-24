use crate::repos::meetings_repo;
use crate::DbError;
use smart_noter_core::models::SearchHit;
use sqlx::SqlitePool;

const MARK_START: char = '\u{2068}';
const MARK_END: char = '\u{2069}';

type MeetingMeta = (String, Option<String>, Option<String>, Option<String>);

/// Turn a user's raw query into a safe FTS5 MATCH expression: each
/// whitespace-separated token becomes a quoted prefix term, so `arq sist`
/// matches `arquitectura sistema`. Quoting neutralizes FTS5 operators
/// (AND/OR/NEAR/"/* etc.). Tokens that are only quote chars collapse to empty
/// and are dropped, so arbitrary input can't be a syntax error and an
/// all-quotes query yields an empty expr (the caller treats that as no results).
fn to_match_expr(query: &str) -> String {
    query
        .split_whitespace()
        .map(|tok| tok.replace('"', ""))
        .filter(|tok| !tok.is_empty())
        .map(|tok| format!("\"{tok}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Rebuild the FTS row for one meeting from its current title/summary/transcript.
pub async fn upsert_meeting(pool: &SqlitePool, meeting_id: &str) -> Result<(), DbError> {
    let meta: Option<MeetingMeta> = sqlx::query_as(
        "SELECT title_es, title_en, summary_es, summary_en FROM meetings WHERE id = ?",
    )
    .bind(meeting_id)
    .fetch_optional(pool)
    .await?;
    let Some((title_es, title_en, summary_es, summary_en)) = meta else {
        return Ok(());
    };

    let lines: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT text_es, text_en FROM transcript_lines WHERE meeting_id = ? ORDER BY t_seconds",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    let title = [Some(title_es), title_en]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    let summary = [summary_es, summary_en]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    let body = lines
        .into_iter()
        .flat_map(|(es, en)| [Some(es), en])
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM meeting_search WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO meeting_search (meeting_id, title, summary, body) VALUES (?, ?, ?, ?)",
    )
    .bind(meeting_id)
    .bind(&title)
    .bind(&summary)
    .bind(&body)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_meeting(pool: &SqlitePool, meeting_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM meeting_search WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Search non-trashed meetings, optionally filtered to a template.
pub async fn search(
    pool: &SqlitePool,
    query: &str,
    template: Option<&str>,
) -> Result<Vec<SearchHit>, DbError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let expr = to_match_expr(query);
    if expr.is_empty() {
        // Every token was quote chars; nothing searchable.
        return Ok(vec![]);
    }
    let mark_start = MARK_START.to_string();
    let mark_end = MARK_END.to_string();

    // Numbered params (?1..?4) so the template bind (?4) can be referenced twice
    // without binding it twice. Filter trashed + optional template; date order
    // keeps it consistent with the normal list.
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT ms.meeting_id, snippet(meeting_search, -1, ?1, ?2, '…', 12) \
         FROM meeting_search ms \
         JOIN meetings m ON m.id = ms.meeting_id \
         WHERE meeting_search MATCH ?3 AND m.deleted_at IS NULL \
           AND (?4 IS NULL OR m.template_id = ?4) \
         ORDER BY m.date DESC",
    )
    .bind(&mark_start)
    .bind(&mark_end)
    .bind(&expr)
    .bind(template)
    .fetch_all(pool)
    .await?;

    let mut hits = Vec::with_capacity(rows.len());
    for (meeting_id, snippet) in rows {
        let meeting = meetings_repo::summary_by_id(pool, &meeting_id).await?;
        if let Some(meeting) = meeting {
            hits.push(SearchHit { meeting, snippet });
        }
    }
    Ok(hits)
}

/// Populate the index for every meeting that has no FTS row yet. Idempotent.
pub async fn backfill(pool: &SqlitePool) -> Result<(), DbError> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM meetings WHERE id NOT IN (SELECT meeting_id FROM meeting_search)",
    )
    .fetch_all(pool)
    .await?;
    for id in ids {
        upsert_meeting(pool, &id).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    async fn seed_meeting(pool: &SqlitePool, id: &str, title: &str) {
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES (?, ?, 'tecnica', '2026-06-01', 1)")
            .bind(id).bind(title).execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn upsert_then_search_finds_by_title_and_body() {
        let pool = init_pool_in_memory().await.unwrap();
        seed_meeting(&pool, "m1", "Revisión de arquitectura").await;
        sqlx::query("INSERT INTO transcript_lines (meeting_id, t_seconds, t_display, text_es) VALUES ('m1', 0, '0:00', 'hablamos del despliegue en kubernetes')")
            .execute(&pool).await.unwrap();
        upsert_meeting(&pool, "m1").await.unwrap();

        let by_title = search(&pool, "arquitectura", None).await.unwrap();
        assert_eq!(by_title.len(), 1);
        assert_eq!(by_title[0].meeting.id, "m1");

        let by_body = search(&pool, "kubernetes", None).await.unwrap();
        assert_eq!(by_body.len(), 1);
        assert!(
            by_body[0].snippet.contains('\u{2068}'),
            "snippet should mark the match"
        );

        let prefix = search(&pool, "kube", None).await.unwrap();
        assert_eq!(prefix.len(), 1, "prefix search should match");
    }

    #[tokio::test]
    async fn search_excludes_trashed_and_respects_template() {
        let pool = init_pool_in_memory().await.unwrap();
        seed_meeting(&pool, "m1", "alpha tecnica").await;
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m2','alpha ejecutiva','ejecutiva','2026-06-01',1)")
            .execute(&pool).await.unwrap();
        upsert_meeting(&pool, "m1").await.unwrap();
        upsert_meeting(&pool, "m2").await.unwrap();

        assert_eq!(search(&pool, "alpha", None).await.unwrap().len(), 2);
        assert_eq!(
            search(&pool, "alpha", Some("tecnica")).await.unwrap().len(),
            1
        );

        sqlx::query("UPDATE meetings SET deleted_at = datetime('now') WHERE id = 'm1'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(search(&pool, "alpha", None).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn weird_query_is_not_a_syntax_error() {
        let pool = init_pool_in_memory().await.unwrap();
        seed_meeting(&pool, "m1", "normal title").await;
        seed_meeting(&pool, "m2", "another title").await;
        upsert_meeting(&pool, "m1").await.unwrap();
        upsert_meeting(&pool, "m2").await.unwrap();
        assert!(search(&pool, "a\"b OR (", None).await.is_ok());

        // A token that is ONLY quote chars must not error and must not match
        // every document (the empty quoted prefix `""*` would do one or the other).
        let only_quotes = search(&pool, "\"\"\"", None).await;
        assert!(
            only_quotes.is_ok(),
            "only-quotes query errored: {only_quotes:?}"
        );
        assert_eq!(
            only_quotes.unwrap().len(),
            0,
            "only-quotes must not match-all"
        );
    }

    #[tokio::test]
    async fn delete_removes_from_index() {
        let pool = init_pool_in_memory().await.unwrap();
        seed_meeting(&pool, "m1", "deletable").await;
        upsert_meeting(&pool, "m1").await.unwrap();
        delete_meeting(&pool, "m1").await.unwrap();
        assert_eq!(search(&pool, "deletable", None).await.unwrap().len(), 0);
    }
}
