use smart_noter_core::{AppError, MeetingAsset};
use sqlx::SqlitePool;

pub struct MeetingAssetsRepo<'a>(pub &'a SqlitePool);

impl MeetingAssetsRepo<'_> {
    pub async fn create(&self, a: &MeetingAsset) -> Result<(), AppError> {
        sqlx::query(
            r#"INSERT INTO meeting_assets (id, meeting_id, kind, path, bytes, mime_type, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&a.id)
        .bind(&a.meeting_id)
        .bind(&a.kind)
        .bind(&a.path)
        .bind(a.bytes)
        .bind(a.mime_type.as_deref())
        .bind(&a.created_at)
        .execute(self.0)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn list_by_meeting(&self, meeting_id: &str) -> Result<Vec<MeetingAsset>, AppError> {
        let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<String>, String)>(
            r#"SELECT id, meeting_id, kind, path, bytes, mime_type, created_at
               FROM meeting_assets WHERE meeting_id = ? ORDER BY created_at"#,
        )
        .bind(meeting_id)
        .fetch_all(self.0)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|(id, meeting_id, kind, path, bytes, mime_type, created_at)| MeetingAsset {
                id,
                meeting_id,
                kind,
                path,
                bytes,
                mime_type,
                created_at,
            })
            .collect())
    }

    pub async fn get_audio(&self, meeting_id: &str) -> Result<Option<MeetingAsset>, AppError> {
        Ok(self
            .list_by_meeting(meeting_id)
            .await?
            .into_iter()
            .find(|a| a.kind == "audio"))
    }

    pub async fn delete(&self, asset_id: &str) -> Result<Option<String>, AppError> {
        let row: Option<(String,)> = sqlx::query_as("SELECT path FROM meeting_assets WHERE id = ?")
            .bind(asset_id)
            .fetch_optional(self.0)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        sqlx::query("DELETE FROM meeting_assets WHERE id = ?")
            .bind(asset_id)
            .execute(self.0)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(row.map(|r| r.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{ensure_schema, in_memory_pool};
    use smart_noter_core::Bilingual;

    async fn seed_meeting(pool: &SqlitePool, id: &str) {
        sqlx::query(r#"INSERT INTO meetings (id, title_es, title_en, template_id, date, duration_sec, device_used, word_count, summary_es, summary_en)
                       VALUES (?, ?, NULL, 'tecnica', '2026-05-19T00:00:00Z', 0, NULL, 0, NULL, NULL)"#)
            .bind(id)
            .bind("test")
            .execute(pool)
            .await
            .unwrap();
        // Keep Bilingual import used to satisfy lint
        let _ = Bilingual { es: "x".into(), en: None };
    }

    fn sample_asset(id: &str, meeting_id: &str) -> MeetingAsset {
        MeetingAsset {
            id: id.into(),
            meeting_id: meeting_id.into(),
            kind: "audio".into(),
            path: "C:/test.wav".into(),
            bytes: 1024,
            mime_type: Some("audio/wav".into()),
            created_at: "2026-05-19T00:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn creates_and_retrieves_asset() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MeetingAssetsRepo(&pool);
        repo.create(&sample_asset("a-1", "m-1")).await.unwrap();
        let assets = repo.list_by_meeting("m-1").await.unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].path, "C:/test.wav");
    }

    #[tokio::test]
    async fn get_audio_returns_only_audio_kind() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MeetingAssetsRepo(&pool);
        let mut transcript = sample_asset("a-1", "m-1");
        transcript.kind = "transcript".into();
        let audio = sample_asset("a-2", "m-1");
        repo.create(&transcript).await.unwrap();
        repo.create(&audio).await.unwrap();
        let got = repo.get_audio("m-1").await.unwrap().unwrap();
        assert_eq!(got.id, "a-2");
    }

    #[tokio::test]
    async fn delete_returns_path_and_removes_row() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MeetingAssetsRepo(&pool);
        repo.create(&sample_asset("a-1", "m-1")).await.unwrap();
        let path = repo.delete("a-1").await.unwrap();
        assert_eq!(path.as_deref(), Some("C:/test.wav"));
        assert!(repo.list_by_meeting("m-1").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn cascade_delete_meetings_drops_assets() {
        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        seed_meeting(&pool, "m-1").await;
        let repo = MeetingAssetsRepo(&pool);
        repo.create(&sample_asset("a-1", "m-1")).await.unwrap();
        sqlx::query("DELETE FROM meetings WHERE id = ?")
            .bind("m-1")
            .execute(&pool)
            .await
            .unwrap();
        assert!(repo.list_by_meeting("m-1").await.unwrap().is_empty());
    }
}
