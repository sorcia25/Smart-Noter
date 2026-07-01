use smart_noter_core::{AppError, Marker};
use sqlx::SqlitePool;

pub struct MarkersRepo<'a>(pub &'a SqlitePool);

impl MarkersRepo<'_> {
    pub async fn list_by_meeting(&self, meeting_id: &str) -> Result<Vec<Marker>, AppError> {
        let rows = sqlx::query_as::<_, (String, String, i64, String, String, String, String)>(
            "SELECT id, meeting_id, t_seconds, kind, label, source, created_at
             FROM markers WHERE meeting_id = ? ORDER BY t_seconds",
        )
        .bind(meeting_id)
        .fetch_all(self.0)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(
                |(id, meeting_id, t_seconds, kind, label, source, created_at)| Marker {
                    id,
                    meeting_id,
                    t_seconds,
                    kind,
                    label,
                    source,
                    created_at,
                },
            )
            .collect())
    }

    pub async fn create(
        &self,
        meeting_id: &str,
        t_seconds: i64,
        kind: &str,
        label: &str,
        source: &str,
    ) -> Result<Marker, AppError> {
        let id = uuid::Uuid::now_v7().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO markers (id, meeting_id, t_seconds, kind, label, source, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(meeting_id)
        .bind(t_seconds)
        .bind(kind)
        .bind(label)
        .bind(source)
        .bind(&created_at)
        .execute(self.0)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(Marker {
            id,
            meeting_id: meeting_id.into(),
            t_seconds,
            kind: kind.into(),
            label: label.into(),
            source: source.into(),
            created_at,
        })
    }

    pub async fn update_label(&self, id: &str, label: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE markers SET label = ? WHERE id = ?")
            .bind(label)
            .bind(id)
            .execute(self.0)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM markers WHERE id = ?")
            .bind(id)
            .execute(self.0)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// Replace all source='ai' markers for a meeting with the given (t, kind, label) triples.
    pub async fn replace_ai(
        &self,
        meeting_id: &str,
        items: &[(i64, String, String)],
    ) -> Result<(), AppError> {
        sqlx::query("DELETE FROM markers WHERE meeting_id = ? AND source = 'ai'")
            .bind(meeting_id)
            .execute(self.0)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        for (t, kind, label) in items {
            self.create(meeting_id, *t, kind, label, "ai").await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{ensure_schema, in_memory_pool};

    async fn seed_meeting(pool: &SqlitePool, id: &str) {
        sqlx::query(r#"INSERT INTO meetings (id, title_es, title_en, template_id, date, duration_sec, device_used, word_count, summary_es, summary_en)
                       VALUES (?, ?, NULL, 'tecnica', '2026-05-19T00:00:00Z', 0, NULL, 0, NULL, NULL)"#)
            .bind(id)
            .bind("test")
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn create_list_and_delete() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MarkersRepo(&pool);
        let m = repo
            .create("m-1", 84, "manual", "Nota", "manual")
            .await
            .unwrap();
        let all = repo.list_by_meeting("m-1").await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].t_seconds, 84);
        repo.delete(&m.id).await.unwrap();
        assert!(repo.list_by_meeting("m-1").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn replace_ai_preserves_manual() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MarkersRepo(&pool);
        repo.create("m-1", 10, "manual", "mine", "manual")
            .await
            .unwrap();
        repo.replace_ai("m-1", &[(20, "decision".into(), "D1".into())])
            .await
            .unwrap();
        let all = repo.list_by_meeting("m-1").await.unwrap();
        assert_eq!(all.len(), 2); // manual kept + 1 ai
        repo.replace_ai("m-1", &[]).await.unwrap();
        let all = repo.list_by_meeting("m-1").await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].source, "manual");
    }

    #[tokio::test]
    async fn list_orders_by_time() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MarkersRepo(&pool);
        repo.create("m-1", 50, "manual", "b", "manual")
            .await
            .unwrap();
        repo.create("m-1", 10, "manual", "a", "manual")
            .await
            .unwrap();
        let all = repo.list_by_meeting("m-1").await.unwrap();
        assert_eq!(all[0].t_seconds, 10);
        assert_eq!(all[1].t_seconds, 50);
    }
}
