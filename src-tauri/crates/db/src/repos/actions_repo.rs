use crate::DbError;
use smart_noter_core::{models::Action, Bilingual};
use sqlx::SqlitePool;

pub async fn list_by_meeting(pool: &SqlitePool, meeting_id: &str) -> Result<Vec<Action>, DbError> {
    let rows = sqlx::query!(
        r#"SELECT id, meeting_id, text_es, text_en, owner_participant_id, due, done
           FROM actions WHERE meeting_id = ?"#,
        meeting_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Action {
            id: r.id,
            meeting_id: r.meeting_id,
            text: Bilingual {
                es: r.text_es,
                en: r.text_en,
            },
            owner_participant_id: r.owner_participant_id,
            due: r.due,
            done: r.done != 0,
        })
        .collect())
}

pub async fn toggle(pool: &SqlitePool, action_id: &str) -> Result<bool, DbError> {
    let row = sqlx::query!("SELECT done FROM actions WHERE id = ?", action_id)
        .fetch_one(pool)
        .await?;
    let new_done = if row.done == 0 { 1 } else { 0 };
    sqlx::query!(
        "UPDATE actions SET done = ? WHERE id = ?",
        new_done,
        action_id
    )
    .execute(pool)
    .await?;
    Ok(new_done != 0)
}

/// Inserts a new action (text in the caller's UI language only; text_en stays
/// NULL until AI translation). Returns the generated id.
pub async fn create(
    pool: &SqlitePool,
    meeting_id: &str,
    text_es: &str,
    owner_participant_id: Option<&str>,
    due: Option<&str>,
) -> Result<String, DbError> {
    let id = format!("act-{}", uuid::Uuid::now_v7());
    sqlx::query(
        "INSERT INTO actions (id, meeting_id, text_es, owner_participant_id, due, done) \
         VALUES (?, ?, ?, ?, ?, 0)",
    )
    .bind(&id)
    .bind(meeting_id)
    .bind(text_es)
    .bind(owner_participant_id)
    .bind(due)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn create_with_source(
    pool: &SqlitePool,
    meeting_id: &str,
    text_es: &str,
    owner_participant_id: Option<&str>,
    due: Option<&str>,
    source: &str,
) -> Result<String, DbError> {
    let id = format!("act-{}", uuid::Uuid::now_v7());
    sqlx::query(
        "INSERT INTO actions (id, meeting_id, text_es, owner_participant_id, due, done, source) \
         VALUES (?, ?, ?, ?, ?, 0, ?)",
    )
    .bind(&id)
    .bind(meeting_id)
    .bind(text_es)
    .bind(owner_participant_id)
    .bind(due)
    .bind(source)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn delete_ai(pool: &SqlitePool, meeting_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM actions WHERE meeting_id = ? AND source = 'ai'")
        .bind(meeting_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update(
    pool: &SqlitePool,
    action_id: &str,
    text_es: &str,
    owner_participant_id: Option<&str>,
    due: Option<&str>,
) -> Result<(), DbError> {
    sqlx::query("UPDATE actions SET text_es = ?, owner_participant_id = ?, due = ? WHERE id = ?")
        .bind(text_es)
        .bind(owner_participant_id)
        .bind(due)
        .bind(action_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, action_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM actions WHERE id = ?")
        .bind(action_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    async fn setup() -> SqlitePool {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query!("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m1', 'M1', 't', '2025-01-01', 100)")
            .execute(&pool).await.unwrap();
        sqlx::query!(
            "INSERT INTO actions (id, meeting_id, text_es, done) VALUES ('a1', 'm1', 'Do thing', 0)"
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn toggle_flips_done() {
        let pool = setup().await;
        let after_first = toggle(&pool, "a1").await.unwrap();
        assert!(after_first);
        let after_second = toggle(&pool, "a1").await.unwrap();
        assert!(!after_second);
    }

    #[tokio::test]
    async fn create_insert_then_lists() {
        let pool = setup().await;
        let id = create(&pool, "m1", "New action", None, None).await.unwrap();
        let list = list_by_meeting(&pool, "m1").await.unwrap();
        assert!(list
            .iter()
            .any(|a| a.id == id && a.text.es == "New action" && !a.done));
    }

    #[tokio::test]
    async fn update_changes_text_owner_due() {
        let pool = setup().await;
        update(&pool, "a1", "Edited", None, Some("2026-07-01"))
            .await
            .unwrap();
        let a = list_by_meeting(&pool, "m1")
            .await
            .unwrap()
            .into_iter()
            .find(|a| a.id == "a1")
            .unwrap();
        assert_eq!(a.text.es, "Edited");
        assert_eq!(a.due.as_deref(), Some("2026-07-01"));
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let pool = setup().await;
        delete(&pool, "a1").await.unwrap();
        assert!(list_by_meeting(&pool, "m1").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_ai_removes_only_ai_actions() {
        let pool = setup().await;
        // a1 is 'manual' (inserted by setup with no source -> DEFAULT 'manual')
        let ai_id = create_with_source(&pool, "m1", "AI action", None, None, "ai")
            .await
            .unwrap();
        let list = list_by_meeting(&pool, "m1").await.unwrap();
        assert_eq!(list.len(), 2);

        delete_ai(&pool, "m1").await.unwrap();

        let list = list_by_meeting(&pool, "m1").await.unwrap();
        assert_eq!(list.len(), 1, "only manual action should remain");
        assert_eq!(list[0].id, "a1");
        assert!(!list.iter().any(|a| a.id == ai_id));
    }
}
