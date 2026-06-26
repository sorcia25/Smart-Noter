use crate::DbError;
use smart_noter_core::models::ai::ChatMessage;
use sqlx::SqlitePool;

/// Insert a new chat message and return its generated id.
pub async fn insert(
    pool: &SqlitePool,
    meeting_id: &str,
    role: &str,
    content: &str,
) -> Result<i64, DbError> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO chat_messages (meeting_id, role, content) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(meeting_id)
    .bind(role)
    .bind(content)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// List all chat messages for a meeting, ordered by id (insertion order).
pub async fn list(pool: &SqlitePool, meeting_id: &str) -> Result<Vec<ChatMessage>, DbError> {
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, role, content, created_at FROM chat_messages \
         WHERE meeting_id = ? ORDER BY id ASC",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, role, content, created_at)| ChatMessage {
            id,
            role,
            content,
            created_at,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    async fn setup() -> SqlitePool {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query(
            "INSERT INTO meetings (id, title_es, template_id, date, duration_sec) \
             VALUES ('m1', 'M1', 't', '2025-01-01', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn insert_and_list_round_trip() {
        let pool = setup().await;

        let id1 = insert(&pool, "m1", "user", "¿De qué trató la reunión?")
            .await
            .unwrap();
        let id2 = insert(&pool, "m1", "assistant", "La reunión trató de X.")
            .await
            .unwrap();

        assert!(id2 > id1, "ids must be monotonically increasing");

        let msgs = list(&pool, "m1").await.unwrap();
        assert_eq!(msgs.len(), 2);

        assert_eq!(msgs[0].id, id1);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content, "¿De qué trató la reunión?");
        assert!(!msgs[0].created_at.is_empty());

        assert_eq!(msgs[1].id, id2);
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].content, "La reunión trató de X.");
    }

    #[tokio::test]
    async fn list_empty_returns_empty_vec() {
        let pool = setup().await;
        let msgs = list(&pool, "m1").await.unwrap();
        assert!(msgs.is_empty());
    }

    #[tokio::test]
    async fn messages_scoped_to_meeting() {
        let pool = setup().await;
        sqlx::query(
            "INSERT INTO meetings (id, title_es, template_id, date, duration_sec) \
             VALUES ('m2', 'M2', 't', '2025-01-02', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        insert(&pool, "m1", "user", "hola desde m1").await.unwrap();
        insert(&pool, "m2", "user", "hola desde m2").await.unwrap();

        let m1_msgs = list(&pool, "m1").await.unwrap();
        let m2_msgs = list(&pool, "m2").await.unwrap();

        assert_eq!(m1_msgs.len(), 1);
        assert_eq!(m2_msgs.len(), 1);
        assert_eq!(m1_msgs[0].content, "hola desde m1");
        assert_eq!(m2_msgs[0].content, "hola desde m2");
    }
}
