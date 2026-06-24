-- Full-text search over meetings: one row per meeting. `meeting_id` is stored
-- but not indexed; title/summary/body are tokenized. `body` holds the meeting's
-- transcript lines concatenated. Maintained explicitly by search_repo (after
-- transcription, title edits) and deleted on purge; trashed meetings stay
-- indexed but are filtered out of results by joining on meetings.deleted_at.
CREATE VIRTUAL TABLE meeting_search USING fts5(
    meeting_id UNINDEXED,
    title,
    summary,
    body
);
