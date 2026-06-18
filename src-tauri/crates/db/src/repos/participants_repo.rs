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

/// Recompute word_count + talk_pct for every participant of a meeting from the
/// current transcript_lines, inside an existing transaction. talk_pct is by
/// speech duration (end_seconds - t_seconds), falling back to word share when
/// total duration is 0. Participants with no lines get 0/0.
async fn recompute_stats_tx(
    tx: &mut sqlx::SqliteConnection,
    meeting_id: &str,
) -> Result<(), DbError> {
    // (participant_id, words, duration) aggregated from lines.
    let rows = sqlx::query_as::<_, (String, i64, i64)>(
        r#"SELECT p.id,
                  COALESCE(SUM((LENGTH(TRIM(tl.text_es)) - LENGTH(REPLACE(TRIM(tl.text_es), ' ', '')) + 1)
                               * (CASE WHEN TRIM(tl.text_es) = '' THEN 0 ELSE 1 END)), 0) AS words,
                  COALESCE(SUM(MAX(COALESCE(tl.end_seconds, tl.t_seconds) - tl.t_seconds, 0)), 0) AS dur
           FROM participants p
           LEFT JOIN transcript_lines tl ON tl.speaker_id = p.id
           WHERE p.meeting_id = ?
           GROUP BY p.id"#,
    )
    .bind(meeting_id)
    .fetch_all(&mut *tx)
    .await?;

    let total_dur: i64 = rows.iter().map(|(_, _, d)| *d).sum();
    let total_words: i64 = rows.iter().map(|(_, w, _)| *w).sum::<i64>().max(1);
    for (id, words, dur) in &rows {
        let pct = if total_dur > 0 {
            ((dur * 100) as f64 / total_dur as f64).round() as i64
        } else {
            (words * 100) / total_words
        };
        sqlx::query("UPDATE participants SET word_count = ?, talk_pct = ? WHERE id = ?")
            .bind(words)
            .bind(pct)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    Ok(())
}

/// Merge `from` into `into`: reassign all of `from`'s lines to `into`, delete
/// `from`, and recompute stats. No-op-safe if `from == into`.
pub async fn merge_speakers(pool: &SqlitePool, into: &str, from: &str) -> Result<(), DbError> {
    if into == from {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE transcript_lines SET speaker_id = ? WHERE speaker_id = ?")
        .bind(into)
        .bind(from)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM participants WHERE id = ?")
        .bind(from)
        .execute(&mut *tx)
        .await?;
    // Recompute for the meeting that owns `into`.
    let meeting_id: String = sqlx::query_scalar("SELECT meeting_id FROM participants WHERE id = ?")
        .bind(into)
        .fetch_one(&mut *tx)
        .await?;
    recompute_stats_tx(&mut tx, &meeting_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Reassign specific lines to `speaker_id` (existing OR newly created → this is
/// "split"). Recomputes stats for the speaker's meeting.
pub async fn reassign_lines(
    pool: &SqlitePool,
    line_ids: &[i64],
    speaker_id: &str,
) -> Result<(), DbError> {
    if line_ids.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    for id in line_ids {
        sqlx::query("UPDATE transcript_lines SET speaker_id = ? WHERE id = ?")
            .bind(speaker_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let meeting_id: String = sqlx::query_scalar("SELECT meeting_id FROM participants WHERE id = ?")
        .bind(speaker_id)
        .fetch_one(&mut *tx)
        .await?;
    recompute_stats_tx(&mut tx, &meeting_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Create a new speaker for a meeting (label S{next}, next free color). Returns its id.
pub async fn create_speaker(pool: &SqlitePool, meeting_id: &str) -> Result<String, DbError> {
    let mut tx = pool.begin().await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM participants WHERE meeting_id = ?")
        .bind(meeting_id)
        .fetch_one(&mut *tx)
        .await?;
    let idx = count as usize; // 0-based
    let id = format!("p-{meeting_id}-S{}", idx + 1);
    let label = format!("S{}", idx + 1);
    let color = format!("s-color-{}", (idx % 8) + 1);
    sqlx::query(
        "INSERT INTO participants (id, meeting_id, label, name, color_class, word_count, talk_pct)
         VALUES (?, ?, ?, NULL, ?, 0, 0)",
    )
    .bind(&id)
    .bind(meeting_id)
    .bind(&label)
    .bind(&color)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
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

    async fn setup_two_speakers() -> SqlitePool {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m','M','tecnica','2025-01-01',100)")
            .execute(&pool).await.unwrap();
        for (id, label, color) in [("p-m-S1", "S1", "s-color-1"), ("p-m-S2", "S2", "s-color-2")] {
            sqlx::query("INSERT INTO participants (id, meeting_id, label, color_class) VALUES (?, 'm', ?, ?)")
                .bind(id).bind(label).bind(color).execute(&pool).await.unwrap();
        }
        // S1: 0..6 (6s, 2 words); S2: 6..10 (4s, 1 word)
        sqlx::query("INSERT INTO transcript_lines (id, meeting_id, t_seconds, end_seconds, t_display, speaker_id, text_es) VALUES (1,'m',0,6,'00:00:00','p-m-S1','hola mundo')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcript_lines (id, meeting_id, t_seconds, end_seconds, t_display, speaker_id, text_es) VALUES (2,'m',6,10,'00:00:06','p-m-S2','adios')")
            .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn merge_reassigns_lines_deletes_from_and_recomputes() {
        let pool = setup_two_speakers().await;
        merge_speakers(&pool, "p-m-S1", "p-m-S2").await.unwrap();
        let speakers: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM participants WHERE meeting_id='m'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(speakers, 1);
        let s1_lines: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM transcript_lines WHERE speaker_id='p-m-S1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(s1_lines, 2);
        let pct: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-S1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(pct, 100);
    }

    #[tokio::test]
    async fn reassign_moves_line_and_recomputes_duration_pct() {
        let pool = setup_two_speakers().await;
        // Move line 1 (the 6s line) to S2 → S2 has 10s of 10s = 100%, S1 = 0%.
        reassign_lines(&pool, &[1], "p-m-S2").await.unwrap();
        let s2: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-S2'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(s2, 100);
        let s1: i64 = sqlx::query_scalar("SELECT talk_pct FROM participants WHERE id='p-m-S1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(s1, 0);
    }

    #[tokio::test]
    async fn create_speaker_adds_next_label_and_color() {
        let pool = setup_two_speakers().await;
        let id = create_speaker(&pool, "m").await.unwrap();
        assert_eq!(id, "p-m-S3");
        let (label, color): (String, String) =
            sqlx::query_as("SELECT label, color_class FROM participants WHERE id='p-m-S3'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(label, "S3");
        assert_eq!(color, "s-color-3");
    }
}
