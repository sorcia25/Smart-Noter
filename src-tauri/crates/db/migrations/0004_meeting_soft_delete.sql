-- Soft delete: NULL deleted_at = active meeting; non-NULL = in Trash.
ALTER TABLE meetings ADD COLUMN deleted_at TEXT;
CREATE INDEX idx_meetings_deleted ON meetings(deleted_at);
