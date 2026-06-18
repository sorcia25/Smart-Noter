-- Sub-3a stored only the start of each line. Diarization talk_pct is computed by
-- real speech duration, so store the segment end too. Nullable: legacy Sub-3a rows
-- have no end and are treated as zero-duration when recomputing.
ALTER TABLE transcript_lines ADD COLUMN end_seconds INTEGER;
