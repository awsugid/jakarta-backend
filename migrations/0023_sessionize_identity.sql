-- Stable Sessionize identity for safe, idempotent daily imports.
-- event_speakers.sessionize_id persists the upstream Sessionize speaker UUID so
-- repeated imports match by identity instead of display name.
ALTER TABLE event_speakers ADD COLUMN sessionize_id TEXT;

-- One Sessionize speaker per event. Partial index: manual speakers
-- (sessionize_id NULL) stay unconstrained and can coexist.
CREATE UNIQUE INDEX IF NOT EXISTS idx_event_speakers_sessionize
  ON event_speakers(event_slug, sessionize_id) WHERE sessionize_id IS NOT NULL;

-- Legacy dedupe before the unique index: pre-0023 syncs could insert the same
-- Sessionize session multiple times per event, so the index below would fail.
-- Every admin row is preserved — younger duplicates only lose their session_id
-- link; the canonical row (lowest rowid = oldest insert per event/session)
-- keeps it. No rows are deleted; NULLed rows become ordinary manual entries.
UPDATE event_schedules
SET session_id = NULL, updated_at = datetime('now')
WHERE session_id IS NOT NULL
  AND rowid <> (
    SELECT MIN(s.rowid) FROM event_schedules AS s
    WHERE s.event_slug = event_schedules.event_slug
      AND s.session_id = event_schedules.session_id
  );

-- Idempotent session imports: at most one schedule row per
-- (event, Sessionize session). Manual rows (session_id NULL) unconstrained.
CREATE UNIQUE INDEX IF NOT EXISTS idx_event_schedules_session
  ON event_schedules(event_slug, session_id) WHERE session_id IS NOT NULL;
