CREATE TABLE meetings (
    id TEXT PRIMARY KEY,
    title_es TEXT NOT NULL,
    title_en TEXT,
    template_id TEXT NOT NULL,
    date TEXT NOT NULL,
    duration_sec INTEGER NOT NULL,
    word_count INTEGER NOT NULL DEFAULT 0,
    device_used TEXT,
    summary_es TEXT,
    summary_en TEXT,
    audio_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_meetings_date ON meetings(date DESC);
CREATE INDEX idx_meetings_template ON meetings(template_id);

CREATE TABLE participants (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    name TEXT,
    color_class TEXT NOT NULL,
    word_count INTEGER NOT NULL DEFAULT 0,
    talk_pct INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_participants_meeting ON participants(meeting_id);

CREATE TABLE actions (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    text_es TEXT NOT NULL,
    text_en TEXT,
    owner_participant_id TEXT REFERENCES participants(id) ON DELETE SET NULL,
    due TEXT,
    done INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_actions_meeting ON actions(meeting_id);

CREATE TABLE transcript_lines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    t_seconds INTEGER NOT NULL,
    t_display TEXT NOT NULL,
    speaker_id TEXT REFERENCES participants(id) ON DELETE SET NULL,
    text_es TEXT NOT NULL,
    text_en TEXT
);

CREATE INDEX idx_transcript_meeting ON transcript_lines(meeting_id, t_seconds);

CREATE TABLE decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    text_es TEXT NOT NULL,
    text_en TEXT
);

CREATE TABLE blockers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    text_es TEXT NOT NULL,
    text_en TEXT
);

CREATE TABLE templates (
    id TEXT PRIMARY KEY,
    color_class TEXT NOT NULL,
    icon TEXT NOT NULL,
    name_es TEXT NOT NULL,
    name_en TEXT NOT NULL,
    desc_es TEXT NOT NULL,
    desc_en TEXT NOT NULL,
    sections_json TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
