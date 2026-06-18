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
///
/// Uses `split_whitespace()` for word counting (same semantics as the write
/// path in `replace_lines`) so tabs/newlines/multiple spaces are handled
/// identically. A SQL approximation would diverge on irregular spacing.
pub(crate) async fn recompute_stats_tx(
    tx: &mut sqlx::SqliteConnection,
    meeting_id: &str,
) -> Result<(), DbError> {
    // Recompute in Rust (NOT SQL) so word-count semantics exactly match the write
    // path (`replace_lines`): `split_whitespace()` counts any whitespace run as one
    // separator. A SQL approximation diverges on tabs/newlines/multiple spaces.
    let participants: Vec<String> =
        sqlx::query_scalar("SELECT id FROM participants WHERE meeting_id = ?")
            .bind(meeting_id)
            .fetch_all(&mut *tx)
            .await?;
    let lines: Vec<(Option<String>, String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT speaker_id, text_es, t_seconds, end_seconds FROM transcript_lines WHERE meeting_id = ?",
    )
    .bind(meeting_id)
    .fetch_all(&mut *tx)
    .await?;

    let mut words: std::collections::HashMap<String, i64> =
        participants.iter().map(|id| (id.clone(), 0)).collect();
    let mut durs: std::collections::HashMap<String, i64> =
        participants.iter().map(|id| (id.clone(), 0)).collect();
    for (speaker_id, text, t_seconds, end_seconds) in &lines {
        let Some(sp) = speaker_id else { continue };
        if let Some(w) = words.get_mut(sp) {
            *w += text.split_whitespace().count() as i64;
        }
        if let Some(d) = durs.get_mut(sp) {
            *d += (end_seconds.unwrap_or(*t_seconds) - t_seconds).max(0);
        }
    }

    let total_dur: i64 = durs.values().sum();
    let total_words: i64 = words.values().sum::<i64>().max(1);
    for id in &participants {
        let w = words[id];
        let d = durs[id];
        let pct = if total_dur > 0 {
            ((d * 100) as f64 / total_dur as f64).round() as i64
        } else {
            (w * 100) / total_words
        };
        sqlx::query("UPDATE participants SET word_count = ?, talk_pct = ? WHERE id = ?")
            .bind(w)
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
        // Guard: only reassign a line that belongs to the SAME meeting as the
        // target speaker — never create a cross-meeting reference.
        sqlx::query(
            "UPDATE transcript_lines SET speaker_id = ?1
             WHERE id = ?2
               AND meeting_id = (SELECT meeting_id FROM participants WHERE id = ?1)",
        )
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
    // Next speaker number = max existing S{n} label suffix + 1 (COUNT-based numbering
    // would collide with a surviving higher-numbered speaker after a merge).
    let labels: Vec<String> =
        sqlx::query_scalar("SELECT label FROM participants WHERE meeting_id = ?")
            .bind(meeting_id)
            .fetch_all(&mut *tx)
            .await?;
    let max_n = labels
        .iter()
        .filter_map(|l| l.strip_prefix('S').and_then(|n| n.parse::<usize>().ok()))
        .max()
        .unwrap_or(0);
    let n = max_n + 1; // 1-based label number
    let id = format!("p-{meeting_id}-S{n}");
    let label = format!("S{n}");
    let color = format!("s-color-{}", ((n - 1) % 8) + 1);
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
    async fn recompute_word_count_matches_split_whitespace_on_irregular_spacing() {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('mw','M','tecnica','2025-01-01',100)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO participants (id, meeting_id, label, color_class) VALUES ('p-mw-S1','mw','S1','s-color-1')")
            .execute(&pool).await.unwrap();
        // Double spaces + a tab: split_whitespace() => 3 words. A single-space SQL
        // count would wrongly report more.
        sqlx::query("INSERT INTO transcript_lines (id, meeting_id, t_seconds, end_seconds, t_display, speaker_id, text_es) VALUES (1,'mw',0,5,'00:00:00','p-mw-S1','hola  mundo\tadios')")
            .execute(&pool).await.unwrap();
        // Trigger a recompute via reassign (no-op move onto the same speaker still recomputes).
        reassign_lines(&pool, &[1], "p-mw-S1").await.unwrap();
        let wc: i64 = sqlx::query_scalar("SELECT word_count FROM participants WHERE id='p-mw-S1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(wc, 3, "recompute must use split_whitespace semantics");
    }

    #[tokio::test]
    async fn reassign_ignores_lines_from_a_different_meeting() {
        let pool = setup_two_speakers().await; // meeting 'm' with p-m-S1, p-m-S2, lines 1 & 2
                                               // A second meeting with its own speaker + line id 99.
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m2','M2','tecnica','2025-01-01',100)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO participants (id, meeting_id, label, color_class) VALUES ('p-m2-S1','m2','S1','s-color-1')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcript_lines (id, meeting_id, t_seconds, end_seconds, t_display, speaker_id, text_es) VALUES (99,'m2',0,5,'00:00:00','p-m2-S1','otra reunion')")
            .execute(&pool).await.unwrap();

        // Try to reassign m2's line 99 to m's speaker p-m-S1 — guard must REJECT it.
        reassign_lines(&pool, &[99], "p-m-S1").await.unwrap();
        let owner: String =
            sqlx::query_scalar("SELECT speaker_id FROM transcript_lines WHERE id=99")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(owner, "p-m2-S1", "cross-meeting reassign must be a no-op");
    }

    #[tokio::test]
    async fn create_speaker_after_merge_does_not_collide() {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('mc','M','tecnica','2025-01-01',100)")
            .execute(&pool).await.unwrap();
        // Speakers S1, S2, S3 exist; merge S2 into S1 leaves S1, S3.
        for (id, label, color) in [
            ("p-mc-S1", "S1", "s-color-1"),
            ("p-mc-S2", "S2", "s-color-2"),
            ("p-mc-S3", "S3", "s-color-3"),
        ] {
            sqlx::query("INSERT INTO participants (id, meeting_id, label, color_class) VALUES (?, 'mc', ?, ?)")
                .bind(id).bind(label).bind(color).execute(&pool).await.unwrap();
        }
        merge_speakers(&pool, "p-mc-S1", "p-mc-S2").await.unwrap(); // now S1, S3
                                                                    // Next speaker must be S4 (max suffix 3 + 1), NOT S3 (which would collide).
        let new_id = create_speaker(&pool, "mc").await.unwrap();
        assert_eq!(new_id, "p-mc-S4");
        let label: String = sqlx::query_scalar("SELECT label FROM participants WHERE id='p-mc-S4'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(label, "S4");
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
