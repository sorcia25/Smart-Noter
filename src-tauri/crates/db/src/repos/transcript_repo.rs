use crate::DbError;
use sqlx::SqlitePool;

/// One transcript line to persist. `speaker_idx` is the 0-based detected speaker
/// (Sub-3a passes 0 for all lines → a single S1). `end_seconds` enables talk_pct
/// by real duration.
#[derive(Debug, Clone)]
pub struct LineInput {
    pub t_seconds: i64,
    pub end_seconds: i64,
    pub t_display: String,
    pub text_es: String,
    pub speaker_idx: usize,
}

/// Color class for the nth (0-based) speaker. The Avatar component defines
/// s-color-1..8 and falls back to s1 beyond that; we cycle 1..=8.
fn color_for(idx: usize) -> String {
    format!("s-color-{}", (idx % 8) + 1)
}

/// Replace a meeting's transcript atomically. Creates exactly `speaker_count`
/// participants (S1..Sn), wipes old lines, inserts the new ones with their
/// speaker, and sets per-speaker word_count + talk_pct (by speech duration).
/// Idempotent. `speaker_count` must be >= 1 and cover every `speaker_idx` used.
pub async fn replace_lines(
    pool: &SqlitePool,
    meeting_id: &str,
    lines: &[LineInput],
    speaker_count: usize,
    word_count: i64,
) -> Result<(), DbError> {
    let speaker_count = speaker_count.max(1);
    let mut tx = pool.begin().await?;

    // Wipe lines first (FK: lines reference participants), then participants.
    sqlx::query("DELETE FROM transcript_lines WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM participants WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;

    // Create S1..Sn.
    let speaker_id = |idx: usize| format!("p-{meeting_id}-S{}", idx + 1);
    for idx in 0..speaker_count {
        sqlx::query(
            "INSERT INTO participants (id, meeting_id, label, name, color_class, word_count, talk_pct)
             VALUES (?, ?, ?, NULL, ?, 0, 0)",
        )
        .bind(speaker_id(idx))
        .bind(meeting_id)
        .bind(format!("S{}", idx + 1))
        .bind(color_for(idx))
        .execute(&mut *tx)
        .await?;
    }

    // Insert lines.
    for l in lines {
        let idx = l.speaker_idx.min(speaker_count - 1);
        sqlx::query(
            "INSERT INTO transcript_lines (meeting_id, t_seconds, end_seconds, t_display, speaker_id, text_es, text_en)
             VALUES (?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(meeting_id)
        .bind(l.t_seconds)
        .bind(l.end_seconds)
        .bind(&l.t_display)
        .bind(speaker_id(idx))
        .bind(&l.text_es)
        .execute(&mut *tx)
        .await?;
    }

    // Recompute per-speaker word_count + talk_pct via the shared helper so that
    // the write path and the correction ops (merge/reassign) always agree.
    crate::repos::participants_repo::recompute_stats_tx(&mut tx, meeting_id).await?;

    sqlx::query("UPDATE meetings SET word_count = ? WHERE id = ?")
        .bind(word_count)
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    #[tokio::test]
    async fn replace_lines_single_speaker_creates_s1_and_word_counts() {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec, word_count) VALUES ('m-1','t','tecnica','2026-06-15',10,0)")
            .execute(&pool).await.unwrap();

        let lines = vec![
            LineInput {
                t_seconds: 0,
                end_seconds: 4,
                t_display: "00:00:00".into(),
                text_es: "hola equipo".into(),
                speaker_idx: 0,
            },
            LineInput {
                t_seconds: 4,
                end_seconds: 8,
                t_display: "00:00:04".into(),
                text_es: "vamos a empezar".into(),
                speaker_idx: 0,
            },
        ];
        replace_lines(&pool, "m-1", &lines, 1, 5).await.unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM transcript_lines WHERE meeting_id='m-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 2);
        let speakers: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM participants WHERE meeting_id='m-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(speakers, 1);
        let pct: i64 =
            sqlx::query_scalar("SELECT talk_pct FROM participants WHERE meeting_id='m-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pct, 100);

        // Re-running replaces, not appends.
        replace_lines(&pool, "m-1", &lines[..1], 1, 2)
            .await
            .unwrap();
        let count2: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM transcript_lines WHERE meeting_id='m-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count2, 1);
    }

    #[tokio::test]
    async fn replace_lines_two_speakers_splits_talk_pct_by_duration() {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec, word_count) VALUES ('m-2','t','tecnica','2026-06-15',10,0)")
            .execute(&pool).await.unwrap();

        // S1 speaks 9s, S2 speaks 3s → 75% / 25%.
        let lines = vec![
            LineInput {
                t_seconds: 0,
                end_seconds: 9,
                t_display: "00:00:00".into(),
                text_es: "uno dos tres".into(),
                speaker_idx: 0,
            },
            LineInput {
                t_seconds: 9,
                end_seconds: 12,
                t_display: "00:00:09".into(),
                text_es: "cuatro".into(),
                speaker_idx: 1,
            },
        ];
        replace_lines(&pool, "m-2", &lines, 2, 4).await.unwrap();

        let speakers: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM participants WHERE meeting_id='m-2'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(speakers, 2);
        let s1: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-2-S1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let s2: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-2-S2'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(s1, 75);
        assert_eq!(s2, 25);
        let colors: Vec<String> = sqlx::query_scalar(
            "SELECT color_class FROM participants WHERE meeting_id='m-2' ORDER BY label",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(colors, vec!["s-color-1", "s-color-2"]);
    }
}
