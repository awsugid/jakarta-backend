-- Event schedules table for managing event timeline, sessions, stages, and imported Sessionize sessions.
CREATE TABLE IF NOT EXISTS event_schedules (
  id TEXT PRIMARY KEY,
  event_slug TEXT NOT NULL,
  session_id TEXT,
  title TEXT NOT NULL,
  description TEXT,
  speaker_id TEXT,
  speaker_name TEXT,
  speaker_role TEXT,
  speaker_company TEXT,
  speaker_avatar TEXT,
  speaker_social_url TEXT,
  room TEXT,
  track TEXT,
  starts_at TEXT,
  ends_at TEXT,
  session_type TEXT DEFAULT 'Technical Session',
  is_service_session INTEGER NOT NULL DEFAULT 0 CHECK (is_service_session IN (0, 1)),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  display_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_event_schedules_event_order
  ON event_schedules(event_slug, display_order, starts_at);
