use crate::{repos::meetings_repo, DbError};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::Path;

#[derive(Deserialize)]
struct SeedData {
    templates: Vec<SeedTemplate>,
    meetings: Vec<SeedMeeting>,
    #[serde(rename = "audioDevices")]
    _audio_devices: serde_json::Value, // unused — devices are hardcoded in command
}

#[derive(Deserialize)]
struct SeedTemplate {
    id: String,
    #[serde(rename = "colorClass")]
    color_class: String,
    icon: String,
    name: BilingualSeed,
    desc: BilingualSeed,
    sections: Vec<String>,
}

#[derive(Deserialize)]
struct BilingualSeed {
    es: String,
    en: String,
}

#[derive(Deserialize)]
struct SeedMeeting {
    id: String,
    title: BilingualSeed,
    template: String,
    date: String,
    #[serde(rename = "durationSec")]
    duration_sec: i64,
    participants: Vec<SeedParticipant>,
    #[serde(rename = "deviceUsed")]
    device_used: Option<String>,
    #[serde(rename = "wordCount", default)]
    word_count: i64,
    #[serde(default)]
    summary: Option<BilingualSeed>,
    #[serde(default)]
    decisions: Vec<BilingualSeed>,
    #[serde(default)]
    blockers: Vec<BilingualSeed>,
    #[serde(default)]
    actions: Vec<SeedAction>,
    #[serde(default)]
    transcript: Vec<SeedTranscriptLine>,
}

#[derive(Deserialize)]
struct SeedParticipant {
    id: String,
    label: String,
    name: Option<String>,
    #[serde(rename = "colorClass")]
    color_class: String,
    #[serde(rename = "wordCount", default)]
    word_count: i64,
    #[serde(rename = "talkPct", default)]
    talk_pct: i64,
}

#[derive(Deserialize)]
struct SeedAction {
    id: String,
    text: BilingualSeed,
    owner: Option<String>,
    due: Option<String>,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize)]
struct SeedTranscriptLine {
    t: String,
    #[serde(rename = "speakerId")]
    speaker_id: String,
    text: BilingualSeed,
}

fn t_seconds(t: &str) -> i64 {
    let parts: Vec<&str> = t.split(':').collect();
    match parts.as_slice() {
        [h, m, s] => {
            h.parse::<i64>().unwrap_or(0) * 3600
                + m.parse::<i64>().unwrap_or(0) * 60
                + s.parse::<i64>().unwrap_or(0)
        }
        [m, s] => m.parse::<i64>().unwrap_or(0) * 60 + s.parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

pub async fn seed_if_empty(pool: &SqlitePool, json_path: &Path) -> Result<(), DbError> {
    let count = meetings_repo::count(pool).await?;
    if count > 0 {
        return Ok(());
    }

    let bytes = std::fs::read(json_path).map_err(|e| DbError::Sqlx(sqlx::Error::Io(e)))?;
    let data: SeedData = serde_json::from_slice(&bytes)
        .map_err(|e| DbError::Sqlx(sqlx::Error::Decode(Box::new(e))))?;

    let mut tx = pool.begin().await?;

    for t in &data.templates {
        let sections = serde_json::to_string(&t.sections).unwrap();
        let is_default: i64 = if t.id == "tecnica" { 1 } else { 0 };
        sqlx::query!(
            "INSERT INTO templates (id, color_class, icon, name_es, name_en, desc_es, desc_en, sections_json, is_default)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            t.id, t.color_class, t.icon, t.name.es, t.name.en, t.desc.es, t.desc.en, sections, is_default
        ).execute(&mut *tx).await?;
    }

    for m in &data.meetings {
        let title_en = Some(m.title.en.clone());
        let summary_es = m.summary.as_ref().map(|s| s.es.clone());
        let summary_en = m.summary.as_ref().map(|s| s.en.clone());
        sqlx::query!(
            "INSERT INTO meetings (id, title_es, title_en, template_id, date, duration_sec, word_count, device_used, summary_es, summary_en)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            m.id, m.title.es, title_en, m.template, m.date, m.duration_sec,
            m.word_count, m.device_used, summary_es, summary_en
        ).execute(&mut *tx).await?;

        for p in &m.participants {
            let unique_id = format!("{}-{}", m.id, p.id);
            sqlx::query!(
                "INSERT INTO participants (id, meeting_id, label, name, color_class, word_count, talk_pct)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                unique_id, m.id, p.label, p.name, p.color_class, p.word_count, p.talk_pct
            ).execute(&mut *tx).await?;
        }

        for d in &m.decisions {
            let en = Some(d.en.clone());
            sqlx::query!(
                "INSERT INTO decisions (meeting_id, text_es, text_en) VALUES (?, ?, ?)",
                m.id,
                d.es,
                en
            )
            .execute(&mut *tx)
            .await?;
        }

        for b in &m.blockers {
            let en = Some(b.en.clone());
            sqlx::query!(
                "INSERT INTO blockers (meeting_id, text_es, text_en) VALUES (?, ?, ?)",
                m.id,
                b.es,
                en
            )
            .execute(&mut *tx)
            .await?;
        }

        for a in &m.actions {
            let unique_action_id = format!("{}-{}", m.id, a.id);
            let owner_id = a.owner.as_ref().map(|o| format!("{}-{}", m.id, o));
            let text_en = Some(a.text.en.clone());
            let done_i: i64 = if a.done { 1 } else { 0 };
            sqlx::query!(
                "INSERT INTO actions (id, meeting_id, text_es, text_en, owner_participant_id, due, done)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                unique_action_id, m.id, a.text.es, text_en, owner_id, a.due, done_i
            ).execute(&mut *tx).await?;
        }

        for line in &m.transcript {
            let seconds = t_seconds(&line.t);
            let speaker_unique = format!("{}-{}", m.id, line.speaker_id);
            let text_en = Some(line.text.en.clone());
            sqlx::query!(
                "INSERT INTO transcript_lines (meeting_id, t_seconds, t_display, speaker_id, text_es, text_en)
                 VALUES (?, ?, ?, ?, ?, ?)",
                m.id, seconds, line.t, speaker_unique, line.text.es, text_en
            ).execute(&mut *tx).await?;
        }
    }

    tx.commit().await?;
    tracing::info!(
        "seeded database with {} meetings, {} templates",
        data.meetings.len(),
        data.templates.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;
    use std::io::Write;

    #[tokio::test]
    async fn seed_is_idempotent() {
        let pool = init_pool_in_memory().await.unwrap();
        let mut f = tempfile::NamedTempFile::new().unwrap();
        let minimal = serde_json::json!({
            "templates": [{
                "id": "tecnica", "colorClass": "t-color-tecnica", "icon": "cpu",
                "name": {"es": "Técnica", "en": "Technical"},
                "desc": {"es": "X", "en": "X"}, "sections": ["actions"]
            }],
            "meetings": [{
                "id": "m1", "title": {"es": "T", "en": "T"}, "template": "tecnica",
                "date": "2025-01-01T00:00:00", "durationSec": 60,
                "participants": [], "actions": []
            }],
            "audioDevices": []
        });
        f.write_all(serde_json::to_string(&minimal).unwrap().as_bytes())
            .unwrap();

        seed_if_empty(&pool, f.path()).await.unwrap();
        let after_first = meetings_repo::count(&pool).await.unwrap();
        seed_if_empty(&pool, f.path()).await.unwrap();
        let after_second = meetings_repo::count(&pool).await.unwrap();

        assert_eq!(after_first, 1);
        assert_eq!(after_second, 1, "seed should be idempotent");
    }
}
