use crate::DbError;
use sqlx::SqlitePool;

/// One transcript line to persist (text goes into the primary `text_es` column).
#[derive(Debug, Clone)]
pub struct LineInput {
    pub t_seconds: i64,
    pub t_display: String,
    pub text_es: String,
}

/// Replace a meeting's transcript atomically: ensure the single `S1` participant,
/// wipe old lines, insert the new ones, set word counts. Idempotent.
pub async fn replace_lines(
    pool: &SqlitePool,
    meeting_id: &str,
    lines: &[LineInput],
    word_count: i64,
) -> Result<(), DbError> {
    let speaker_id = format!("p-{meeting_id}-S1");
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT OR IGNORE INTO participants (id, meeting_id, label, name, color_class, word_count, talk_pct)
         VALUES (?, ?, 'S1', NULL, 's-color-1', 0, 100)",
    )
    .bind(&speaker_id)
    .bind(meeting_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM transcript_lines WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(&mut *tx)
        .await?;

    for l in lines {
        sqlx::query(
            "INSERT INTO transcript_lines (meeting_id, t_seconds, t_display, speaker_id, text_es, text_en)
             VALUES (?, ?, ?, ?, ?, NULL)",
        )
        .bind(meeting_id)
        .bind(l.t_seconds)
        .bind(&l.t_display)
        .bind(&speaker_id)
        .bind(&l.text_es)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("UPDATE participants SET word_count = ?, talk_pct = 100 WHERE id = ?")
        .bind(word_count)
        .bind(&speaker_id)
        .execute(&mut *tx)
        .await?;

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
    async fn replace_lines_creates_s1_inserts_lines_and_sets_word_count() {
        let pool = init_pool_in_memory().await.unwrap();
        // seed a meeting row (minimal columns; mirror the meetings insert in meetings_repo tests)
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec, word_count) VALUES ('m-1','t','tecnica','2026-06-15',10,0)")
            .execute(&pool)
            .await
            .unwrap();

        let lines = vec![
            LineInput {
                t_seconds: 0,
                t_display: "00:00:00".into(),
                text_es: "hola equipo".into(),
            },
            LineInput {
                t_seconds: 4,
                t_display: "00:00:04".into(),
                text_es: "vamos a empezar".into(),
            },
        ];
        replace_lines(&pool, "m-1", &lines, 5).await.unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM transcript_lines WHERE meeting_id='m-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 2);
        let wc: i64 = sqlx::query_scalar("SELECT word_count FROM meetings WHERE id='m-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(wc, 5);
        let speakers: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM participants WHERE meeting_id='m-1' AND label='S1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(speakers, 1);

        // Re-running replaces, not appends, and keeps a single S1.
        replace_lines(&pool, "m-1", &lines[..1], 2).await.unwrap();
        let count2: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM transcript_lines WHERE meeting_id='m-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count2, 1);
        let speakers2: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM participants WHERE meeting_id='m-1' AND label='S1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(speakers2, 1);
    }
}
