use crate::DbError;
use smart_noter_core::models::Participant;
use sqlx::SqlitePool;

pub async fn list_by_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<Participant>, DbError> {
    let rows = sqlx::query_as!(
        Participant,
        r#"SELECT id, meeting_id, label, name, color_class, word_count, talk_pct
           FROM participants WHERE meeting_id = ? ORDER BY label"#,
        meeting_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn rename(
    pool: &SqlitePool,
    participant_id: &str,
    name: Option<&str>,
) -> Result<(), DbError> {
    sqlx::query!(
        "UPDATE participants SET name = ? WHERE id = ?",
        name,
        participant_id
    )
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
        sqlx::query!(
            "INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m1', 'M1', 'tecnica', '2025-01-01T00:00:00', 100)"
        ).execute(&pool).await.unwrap();
        sqlx::query!(
            "INSERT INTO participants (id, meeting_id, label, color_class) VALUES ('p1', 'm1', 'S1', 's-color-1')"
        ).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn rename_persists() {
        let pool = setup().await;
        rename(&pool, "p1", Some("Alice")).await.unwrap();
        let parts = list_by_meeting(&pool, "m1").await.unwrap();
        assert_eq!(parts[0].name.as_deref(), Some("Alice"));
    }

    #[tokio::test]
    async fn rename_to_none_clears_name() {
        let pool = setup().await;
        rename(&pool, "p1", Some("Alice")).await.unwrap();
        rename(&pool, "p1", None).await.unwrap();
        let parts = list_by_meeting(&pool, "m1").await.unwrap();
        assert_eq!(parts[0].name, None);
    }
}
