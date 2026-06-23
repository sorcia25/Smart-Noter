use crate::DbError;
use sqlx::SqlitePool;

pub async fn create(pool: &SqlitePool, meeting_id: &str, text_es: &str) -> Result<i64, DbError> {
    let row: (i64,) =
        sqlx::query_as("INSERT INTO decisions (meeting_id, text_es) VALUES (?, ?) RETURNING id")
            .bind(meeting_id)
            .bind(text_es)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

pub async fn update(pool: &SqlitePool, id: i64, text_es: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE decisions SET text_es = ? WHERE id = ?")
        .bind(text_es)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), DbError> {
    sqlx::query("DELETE FROM decisions WHERE id = ?")
        .bind(id)
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
        let r: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM decisions")
            .fetch_one(pool)
            .await
            .unwrap();
        r.0
    }

    #[tokio::test]
    async fn create_update_delete_round_trip() {
        let pool = setup().await;
        let id = create(&pool, "m1", "Adopt X").await.unwrap();
        assert_eq!(count(&pool).await, 1);

        update(&pool, id, "Adopt Y").await.unwrap();
        let txt: (String,) = sqlx::query_as("SELECT text_es FROM decisions WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(txt.0, "Adopt Y");

        delete(&pool, id).await.unwrap();
        assert_eq!(count(&pool).await, 0);
    }
}
