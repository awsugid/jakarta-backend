-- Customizable talk formats (event-specific, like rooms): add / rename /
-- reorder / archive. Replaces hardcoded sessionType choices; session_type on
-- event_schedules stays as a display-label compatibility column.

CREATE TABLE IF NOT EXISTS event_formats (
  id TEXT PRIMARY KEY,
  event_slug TEXT NOT NULL,
  name TEXT NOT NULL,
  display_order INTEGER NOT NULL DEFAULT 0,
  archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
  revision INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (event_slug, name)
);

ALTER TABLE event_schedules ADD COLUMN format_id TEXT;

-- Seed one format per distinct legacy session_type label; earliest usage wins ordering.
INSERT INTO event_formats (id, event_slug, name, display_order, archived, revision, created_at, updated_at)
SELECT 'fmt-' || lower(hex(randomblob(8))), event_slug, session_type,
       ROW_NUMBER() OVER (PARTITION BY event_slug ORDER BY first_seen),
       0, 1, datetime('now'), datetime('now')
FROM (
  SELECT event_slug, session_type, MIN(rowid) AS first_seen
  FROM event_schedules
  WHERE session_type IS NOT NULL AND trim(session_type) <> ''
  GROUP BY event_slug, session_type
);

-- Link legacy rows to seeded formats by label.
UPDATE event_schedules
SET format_id = (
  SELECT f.id FROM event_formats f
  WHERE f.event_slug = event_schedules.event_slug AND f.name = event_schedules.session_type
)
WHERE session_type IS NOT NULL AND trim(session_type) <> '';
