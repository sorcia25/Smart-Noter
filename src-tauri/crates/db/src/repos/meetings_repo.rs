use crate::repos::{actions_repo, participants_repo};
use crate::DbError;
use smart_noter_core::{
    models::{MeetingDetail, MeetingSummary, TranscriptLine},
    Bilingual,
};
use sqlx::SqlitePool;

/// Shared builder: runs `sql` (which must SELECT the standard summary columns)
/// and hydrates each row's participants.
async fn summaries_from_sql(pool: &SqlitePool, sql: &str) -> Result<Vec<MeetingSummary>, DbError> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, i64, i64)>(sql)
        .fetch_all(pool)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, title_es, title_en, template, date, duration_sec, word_count) in rows {
        let participants = participants_repo::list_by_meeting(pool, &id).await?;
        out.push(MeetingSummary {
            id,
            title: Bilingual {
                es: title_es,
                en: title_en,
            },
            template,
            date,
            duration_sec,
            participants,
            word_count,
        });
    }
    Ok(out)
}

type SummaryRow = (String, String, Option<String>, String, String, i64, i64);

/// Returns the summary for a single non-trashed meeting, or `None` if not found.
/// Follows the same column order and participant-hydration pattern as
/// `summaries_from_sql`.
pub async fn summary_by_id(pool: &SqlitePool, id: &str) -> Result<Option<MeetingSummary>, DbError> {
    let row: Option<SummaryRow> = sqlx::query_as(
        "SELECT id, title_es, title_en, template_id, date, duration_sec, word_count \
             FROM meetings WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let Some((id, title_es, title_en, template, date, duration_sec, word_count)) = row else {
        return Ok(None);
    };
    let participants = participants_repo::list_by_meeting(pool, &id).await?;
    Ok(Some(MeetingSummary {
        id,
        title: Bilingual {
            es: title_es,
            en: title_en,
        },
        template,
        date,
        duration_sec,
        participants,
        word_count,
    }))
}

pub async fn list_summaries(pool: &SqlitePool) -> Result<Vec<MeetingSummary>, DbError> {
    summaries_from_sql(
        pool,
        r#"SELECT id, title_es, title_en, template_id, date, duration_sec, word_count
           FROM meetings WHERE deleted_at IS NULL ORDER BY date DESC"#,
    )
    .await
}

pub async fn list_trashed(pool: &SqlitePool) -> Result<Vec<MeetingSummary>, DbError> {
    summaries_from_sql(
        pool,
        r#"SELECT id, title_es, title_en, template_id, date, duration_sec, word_count
           FROM meetings WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC"#,
    )
    .await
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

    let decisions = sqlx::query_as::<_, (i64, String, Option<String>)>(
        "SELECT id, text_es, text_en FROM decisions WHERE meeting_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(
        |(id, text_es, text_en)| smart_noter_core::models::Decision {
            id,
            text: Bilingual {
                es: text_es,
                en: text_en,
            },
        },
    )
    .collect();

    let blockers = sqlx::query_as::<_, (i64, String, Option<String>)>(
        "SELECT id, text_es, text_en FROM blockers WHERE meeting_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, text_es, text_en)| smart_noter_core::models::Blocker {
        id,
        text: Bilingual {
            es: text_es,
            en: text_en,
        },
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

pub async fn update_summary(
    pool: &SqlitePool,
    id: &str,
    summary: &Bilingual,
) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE meetings SET summary_es = ?, summary_en = ?, summarized_at = datetime('now') WHERE id = ?",
    )
    .bind(&summary.es)
    .bind(summary.en.as_deref())
    .bind(id)
    .execute(pool)
    .await
    .map_err(DbError::from)?;
    Ok(())
}

pub async fn soft_delete(pool: &SqlitePool, id: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE meetings SET deleted_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn restore(pool: &SqlitePool, id: &str) -> Result<(), DbError> {
    sqlx::query("UPDATE meetings SET deleted_at = NULL WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Hard-deletes the meeting (CASCADE wipes participants/actions/decisions/
/// blockers/transcript_lines/meeting_assets) and returns its audio asset
/// path(s) so the caller can unlink the files from disk. Only meetings already
/// in the trash (`deleted_at IS NOT NULL`) are purged; calling this on an active
/// meeting is a no-op that returns an empty path list. The path SELECT and the
/// DELETE run in one transaction so the returned paths always correspond to a
/// row that was actually removed.
pub async fn purge(pool: &SqlitePool, id: &str) -> Result<Vec<String>, DbError> {
    let mut tx = pool.begin().await?;

    let paths: Vec<String> = sqlx::query_scalar(
        "SELECT a.path FROM meeting_assets a \
         JOIN meetings m ON m.id = a.meeting_id \
         WHERE a.meeting_id = ? AND a.kind = 'audio' AND m.deleted_at IS NOT NULL",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM meetings WHERE id = ? AND deleted_at IS NOT NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(paths)
}

/// IDs of meetings trashed more than 30 days ago.
pub async fn list_purgeable(pool: &SqlitePool) -> Result<Vec<String>, DbError> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM meetings WHERE deleted_at IS NOT NULL \
         AND deleted_at < datetime('now','-30 days')",
    )
    .fetch_all(pool)
    .await?;
    Ok(ids)
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

    async fn insert_meeting(pool: &SqlitePool, id: &str, deleted_at: Option<&str>) {
        sqlx::query(
            r#"INSERT INTO meetings (id, title_es, template_id, date, duration_sec, deleted_at)
               VALUES (?, ?, 'tecnica', '2026-06-01T00:00:00Z', 10, ?)"#,
        )
        .bind(id)
        .bind(format!("M {id}"))
        .bind(deleted_at)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn list_summaries_excludes_trashed() {
        let pool = init_pool_in_memory().await.unwrap();
        insert_meeting(&pool, "m-active", None).await;
        insert_meeting(&pool, "m-trashed", Some("2026-06-02T00:00:00Z")).await;

        let active = list_summaries(&pool).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "m-active");
    }

    #[tokio::test]
    async fn list_trashed_returns_only_trashed() {
        let pool = init_pool_in_memory().await.unwrap();
        insert_meeting(&pool, "m-active", None).await;
        insert_meeting(&pool, "m-trashed", Some("2026-06-02T00:00:00Z")).await;

        let trashed = list_trashed(&pool).await.unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0].id, "m-trashed");
    }

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

    #[tokio::test]
    async fn soft_delete_then_restore_round_trips() {
        let pool = init_pool_in_memory().await.unwrap();
        insert_meeting(&pool, "m1", None).await;

        soft_delete(&pool, "m1").await.unwrap();
        assert_eq!(list_summaries(&pool).await.unwrap().len(), 0);
        assert_eq!(list_trashed(&pool).await.unwrap().len(), 1);

        restore(&pool, "m1").await.unwrap();
        assert_eq!(list_summaries(&pool).await.unwrap().len(), 1);
        assert_eq!(list_trashed(&pool).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn purge_deletes_row_and_returns_audio_paths() {
        use crate::repos::MeetingsRepo;
        use smart_noter_core::{Bilingual, MeetingAsset, MeetingDetail};

        let pool = init_pool_in_memory().await.unwrap();
        let meeting = MeetingDetail {
            id: "m-purge".into(),
            title: Bilingual {
                es: "P".into(),
                en: None,
            },
            template: "tecnica".into(),
            date: "2026-06-01T00:00:00Z".into(),
            duration_sec: 5,
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
            id: "a-purge".into(),
            meeting_id: "m-purge".into(),
            kind: "audio".into(),
            path: "C:/audio/m-purge.wav".into(),
            bytes: 1,
            mime_type: Some("audio/wav".into()),
            created_at: "2026-06-01T00:00:00Z".into(),
        };
        MeetingsRepo(&pool)
            .create_with_asset(&meeting, &asset)
            .await
            .unwrap();
        soft_delete(&pool, "m-purge").await.unwrap();

        let paths = purge(&pool, "m-purge").await.unwrap();
        assert_eq!(paths, vec!["C:/audio/m-purge.wav".to_string()]);
        assert_eq!(count(&pool).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn purge_ignores_active_meeting() {
        let pool = init_pool_in_memory().await.unwrap();
        insert_meeting(&pool, "m-active", None).await;

        // Not trashed -> purge must delete nothing and return no paths.
        let paths = purge(&pool, "m-active").await.unwrap();
        assert!(paths.is_empty());
        assert_eq!(count(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn list_purgeable_returns_old_trash_only() {
        let pool = init_pool_in_memory().await.unwrap();
        // 40 days ago -> purgeable; just-now trash -> not.
        insert_meeting(&pool, "m-old", None).await;
        sqlx::query(
            "UPDATE meetings SET deleted_at = datetime('now','-40 days') WHERE id = 'm-old'",
        )
        .execute(&pool)
        .await
        .unwrap();
        insert_meeting(&pool, "m-fresh", None).await;
        soft_delete(&pool, "m-fresh").await.unwrap();

        let ids = list_purgeable(&pool).await.unwrap();
        assert_eq!(ids, vec!["m-old".to_string()]);
    }

    #[tokio::test]
    async fn update_summary_writes_summary_and_summarized_at() {
        let pool = init_pool_in_memory().await.unwrap();
        insert_meeting(&pool, "m-sum", None).await;

        let summary = Bilingual {
            es: "Resumen de prueba".into(),
            en: Some("Test summary".into()),
        };
        update_summary(&pool, "m-sum", &summary).await.unwrap();

        // Confirm summary_es / summary_en written via get_detail.
        let detail = get_detail(&pool, "m-sum").await.unwrap();
        let s = detail.summary.expect("summary should be Some");
        assert_eq!(s.es, "Resumen de prueba");
        assert_eq!(s.en.as_deref(), Some("Test summary"));

        // Confirm summarized_at is non-null via a direct SELECT.
        let at: Option<String> =
            sqlx::query_scalar("SELECT summarized_at FROM meetings WHERE id = 'm-sum'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            at.is_some(),
            "summarized_at should be non-null after update_summary"
        );
    }
}
