use crate::repos::{actions_repo, participants_repo};
use crate::DbError;
use smart_noter_core::{
    models::{MeetingDetail, MeetingSummary, TranscriptLine},
    Bilingual,
};
use sqlx::SqlitePool;

pub async fn list_summaries(pool: &SqlitePool) -> Result<Vec<MeetingSummary>, DbError> {
    let rows = sqlx::query!(
        r#"SELECT id, title_es, title_en, template_id, date, duration_sec, word_count
           FROM meetings ORDER BY date DESC"#
    )
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let participants = participants_repo::list_by_meeting(pool, &r.id).await?;
        out.push(MeetingSummary {
            id: r.id,
            title: Bilingual {
                es: r.title_es,
                en: r.title_en,
            },
            template: r.template_id,
            date: r.date,
            duration_sec: r.duration_sec,
            participants,
            word_count: r.word_count,
        });
    }
    Ok(out)
}

pub async fn get_detail(pool: &SqlitePool, id: &str) -> Result<MeetingDetail, DbError> {
    let m = sqlx::query!(
        r#"SELECT id, title_es, title_en, template_id, date, duration_sec, word_count,
                  device_used, summary_es, summary_en
           FROM meetings WHERE id = ?"#,
        id
    )
    .fetch_one(pool)
    .await?;

    let participants = participants_repo::list_by_meeting(pool, id).await?;
    let actions = actions_repo::list_by_meeting(pool, id).await?;

    let decisions = sqlx::query!(
        "SELECT text_es, text_en FROM decisions WHERE meeting_id = ?",
        id
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| Bilingual {
        es: r.text_es,
        en: r.text_en,
    })
    .collect();

    let blockers = sqlx::query!(
        "SELECT text_es, text_en FROM blockers WHERE meeting_id = ?",
        id
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| Bilingual {
        es: r.text_es,
        en: r.text_en,
    })
    .collect();

    let transcript: Vec<TranscriptLine> = sqlx::query_as::<_, (i64, String, Option<String>, String, Option<String>)>(
        "SELECT id, t_display, speaker_id, text_es, text_en FROM transcript_lines WHERE meeting_id = ? ORDER BY t_seconds",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, t_display, speaker_id, text_es, text_en)| TranscriptLine {
        id,
        t: t_display,
        speaker_id: speaker_id.unwrap_or_default(),
        text: Bilingual { es: text_es, en: text_en },
    })
    .collect();

    let summary = match (m.summary_es, m.summary_en) {
        (Some(es), en) => Some(Bilingual { es, en }),
        _ => None,
    };

    Ok(MeetingDetail {
        id: m.id,
        title: Bilingual {
            es: m.title_es,
            en: m.title_en,
        },
        template: m.template_id,
        date: m.date,
        duration_sec: m.duration_sec,
        device_used: m.device_used,
        word_count: m.word_count,
        summary,
        participants,
        actions,
        decisions,
        blockers,
        transcript,
    })
}

pub async fn update_title(
    pool: &SqlitePool,
    id: &str,
    title_es: &str,
    title_en: Option<&str>,
) -> Result<(), DbError> {
    sqlx::query!(
        "UPDATE meetings SET title_es = ?, title_en = ?, updated_at = datetime('now') WHERE id = ?",
        title_es,
        title_en,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count(pool: &SqlitePool) -> Result<i64, DbError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meetings")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub struct MeetingsRepo<'a>(pub &'a sqlx::SqlitePool);

impl MeetingsRepo<'_> {
    /// Atomically inserts a meeting row + a meeting-asset row in a single
    /// SQL transaction. Either both rows persist or neither does.
    ///
    /// NOTE: Only the scalar meeting columns (`id`, `title_es/_en`, `template_id`,
    /// `date`, `duration_sec`, `device_used`, `word_count`, `summary_es/_en`)
    /// are persisted. The bilingual relation fields on `MeetingDetail`
    /// (`participants`, `actions`, `decisions`, `blockers`, `transcript`) are
    /// IGNORED — they must be inserted separately via their respective repos.
    pub async fn create_with_asset(
        &self,
        meeting: &smart_noter_core::MeetingDetail,
        asset: &smart_noter_core::MeetingAsset,
    ) -> Result<(), smart_noter_core::AppError> {
        let mut tx = self
            .0
            .begin()
            .await
            .map_err(|e| smart_noter_core::AppError::Database(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO meetings (id, title_es, title_en, template_id, date, duration_sec,
                                     device_used, word_count, summary_es, summary_en)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&meeting.id)
        .bind(&meeting.title.es)
        .bind(meeting.title.en.as_deref())
        .bind(&meeting.template)
        .bind(&meeting.date)
        .bind(meeting.duration_sec)
        .bind(meeting.device_used.as_deref())
        .bind(meeting.word_count)
        .bind(meeting.summary.as_ref().map(|s| s.es.clone()))
        .bind(meeting.summary.as_ref().and_then(|s| s.en.clone()))
        .execute(&mut *tx)
        .await
        .map_err(|e| smart_noter_core::AppError::Database(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO meeting_assets (id, meeting_id, kind, path, bytes, mime_type, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&asset.id)
        .bind(&asset.meeting_id)
        .bind(&asset.kind)
        .bind(&asset.path)
        .bind(asset.bytes)
        .bind(asset.mime_type.as_deref())
        .bind(&asset.created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| smart_noter_core::AppError::Database(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| smart_noter_core::AppError::Database(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    #[tokio::test]
    async fn list_summaries_empty_on_fresh_db() {
        let pool = init_pool_in_memory().await.unwrap();
        assert!(list_summaries(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn count_zero_on_fresh_db() {
        let pool = init_pool_in_memory().await.unwrap();
        assert_eq!(count(&pool).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn create_with_asset_writes_both_rows_atomically() {
        use crate::connection::{ensure_schema, in_memory_pool};
        use crate::repos::MeetingAssetsRepo;
        use smart_noter_core::{Bilingual, MeetingAsset, MeetingDetail};

        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        let repo = MeetingsRepo(&pool);

        let meeting = MeetingDetail {
            id: "m-tx-1".into(),
            title: Bilingual {
                es: "TX test".into(),
                en: None,
            },
            template: "tecnica".into(),
            date: "2026-05-19T00:00:00Z".into(),
            duration_sec: 42,
            device_used: None,
            word_count: 0,
            summary: None,
            participants: vec![],
            actions: vec![],
            decisions: vec![],
            blockers: vec![],
            transcript: vec![],
        };
        let asset = MeetingAsset {
            id: "a-tx-1".into(),
            meeting_id: "m-tx-1".into(),
            kind: "audio".into(),
            path: "C:/tx.wav".into(),
            bytes: 999,
            mime_type: Some("audio/wav".into()),
            created_at: "2026-05-19T00:00:00Z".into(),
        };

        repo.create_with_asset(&meeting, &asset).await.unwrap();

        let assets = MeetingAssetsRepo(&pool)
            .list_by_meeting("m-tx-1")
            .await
            .unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].id, "a-tx-1");
    }

    #[tokio::test]
    async fn create_with_asset_rolls_back_meeting_when_asset_insert_fails() {
        use crate::connection::{ensure_schema, in_memory_pool};
        use smart_noter_core::{Bilingual, MeetingAsset, MeetingDetail};

        let pool = in_memory_pool().await.unwrap();
        ensure_schema(&pool).await.unwrap();
        let repo = MeetingsRepo(&pool);

        let meeting = MeetingDetail {
            id: "m-rollback-1".into(),
            title: Bilingual {
                es: "Rollback test".into(),
                en: None,
            },
            template: "tecnica".into(),
            date: "2026-05-19T00:00:00Z".into(),
            duration_sec: 1,
            device_used: None,
            word_count: 0,
            summary: None,
            participants: vec![],
            actions: vec![],
            decisions: vec![],
            blockers: vec![],
            transcript: vec![],
        };
        // Asset with invalid `kind` — violates the CHECK constraint on
        // meeting_assets.kind, forcing the second INSERT to fail.
        let bad_asset = MeetingAsset {
            id: "a-rollback-1".into(),
            meeting_id: "m-rollback-1".into(),
            kind: "invalid_kind".into(),
            path: "C:/never.wav".into(),
            bytes: 0,
            mime_type: None,
            created_at: "2026-05-19T00:00:00Z".into(),
        };

        let result = repo.create_with_asset(&meeting, &bad_asset).await;
        assert!(result.is_err(), "expected CHECK constraint violation");

        // The meeting must NOT exist — the failed asset insert should have
        // rolled back the preceding meeting insert.
        assert_eq!(
            count(&pool).await.unwrap(),
            0,
            "meeting row leaked despite asset insert failure"
        );
    }
}
