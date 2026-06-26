use crate::DbError;
use sqlx::SqlitePool;

pub async fn create(pool: &SqlitePool, meeting_id: &str, text_es: &str) -> Result<i64, DbError> {
    let row: (i64,) =
        sqlx::query_as("INSERT INTO blockers (meeting_id, text_es) VALUES (?, ?) RETURNING id")
            .bind(meeting_id)
            .bind(text_es)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

pub async fn update(pool: &SqlitePool, id: i64, text_es: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE blockers SET text_es = ? WHERE id = ?")
        .bind(text_es)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), DbError> {
    sqlx::query("DELETE FROM blockers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn create_with_source(
    pool: &SqlitePool,
    meeting_id: &str,
    text_es: &str,
    source: &str,
) -> Result<i64, DbError> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO blockers (meeting_id, text_es, source) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(meeting_id)
    .bind(text_es)
    .bind(source)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn delete_ai(pool: &SqlitePool, meeting_id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM blockers WHERE meeting_id = ? AND source = 'ai'")
        .bind(meeting_id)
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
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m1','M1','t','2025-01-01',1)")
            .execute(&pool).await.unwrap();
        pool
    }
    async fn count(pool: &SqlitePool) -> i64 {
        let r: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM blockers")
            .fetch_one(pool)
            .await
            .unwrap();
        r.0
    }

    #[tokio::test]
    async fn create_update_delete_round_trip() {
        let pool = setup().await;
        let id = create(&pool, "m1", "Blocked on infra").await.unwrap();
        assert_eq!(count(&pool).await, 1);
        update(&pool, id, "Unblocked-ish").await.unwrap();
        let txt: (String,) = sqlx::query_as("SELECT text_es FROM blockers WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(txt.0, "Unblocked-ish");
        delete(&pool, id).await.unwrap();
        assert_eq!(count(&pool).await, 0);
    }

    #[tokio::test]
    async fn delete_ai_removes_only_ai_blockers() {
        let pool = setup().await;
        create_with_source(&pool, "m1", "AI blocker", "ai")
            .await
            .unwrap();
        create_with_source(&pool, "m1", "Manual blocker", "manual")
            .await
            .unwrap();
        assert_eq!(count(&pool).await, 2);

        delete_ai(&pool, "m1").await.unwrap();
        assert_eq!(count(&pool).await, 1);

        let txt: (String,) = sqlx::query_as("SELECT text_es FROM blockers")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(txt.0, "Manual blocker");
    }
}
