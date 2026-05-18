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
}
