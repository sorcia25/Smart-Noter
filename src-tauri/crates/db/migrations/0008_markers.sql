-- Audio markers (manual + AI-anchored). kind: decision|action|blocker|highlight|manual
CREATE TABLE markers (
    id         TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    t_seconds  INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    label      TEXT NOT NULL,
    source     TEXT NOT NULL,   -- 'ai' | 'manual'
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_markers_meeting ON markers(meeting_id);
