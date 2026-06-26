-- AI summary + chat (Sub-5).
ALTER TABLE meetings ADD COLUMN summarized_at TEXT;            -- NULL = never summarized
ALTER TABLE decisions ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';  -- 'ai' | 'manual'
ALTER TABLE blockers  ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE actions   ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';

CREATE TABLE chat_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    role TEXT NOT NULL,            -- 'user' | 'assistant'
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_chat_meeting ON chat_messages(meeting_id);

CREATE TABLE transcript_embeddings (
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    chunk_idx INTEGER NOT NULL,
    text TEXT NOT NULL,
    vector BLOB NOT NULL,         -- f32 little-endian, length = dim
    PRIMARY KEY (meeting_id, chunk_idx)
);
