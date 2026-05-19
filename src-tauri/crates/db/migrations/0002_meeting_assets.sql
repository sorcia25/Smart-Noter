CREATE TABLE meeting_assets (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('audio', 'transcript', 'export')),
    path TEXT NOT NULL,
    bytes INTEGER NOT NULL CHECK (bytes >= 0),
    mime_type TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_meeting_assets_meeting_id ON meeting_assets(meeting_id);
CREATE INDEX idx_meeting_assets_kind ON meeting_assets(meeting_id, kind);
