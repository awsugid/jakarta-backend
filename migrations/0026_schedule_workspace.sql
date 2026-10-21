-- Schedule workspace: configurable event rooms, room/speaker-id linkage and
-- optimistic-concurrency revisions on event_schedules, plus a per-event
-- workspace revision that invalidates review/publish tokens on any mutation
-- relevant to the published snapshot (items, rooms, speaker directory).

CREATE TABLE IF NOT EXISTS event_rooms (
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

ALTER TABLE event_schedules ADD COLUMN room_ids_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE event_schedules ADD COLUMN all_rooms INTEGER NOT NULL DEFAULT 0 CHECK (all_rooms IN (0, 1));
ALTER TABLE event_schedules ADD COLUMN speaker_ids_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE event_schedules ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;

CREATE TABLE IF NOT EXISTS event_schedule_workspace (
  event_slug TEXT PRIMARY KEY,
  revision INTEGER NOT NULL DEFAULT 1
);

-- Seed one room per distinct legacy `room` name; earliest usage wins ordering.
INSERT INTO event_rooms (id, event_slug, name, display_order, archived, revision, created_at, updated_at)
SELECT 'room-' || lower(hex(randomblob(8))), event_slug, room,
       ROW_NUMBER() OVER (PARTITION BY event_slug ORDER BY first_seen),
       0, 1, datetime('now'), datetime('now')
FROM (
  SELECT event_slug, room, MIN(rowid) AS first_seen
  FROM event_schedules
  WHERE room IS NOT NULL AND trim(room) <> ''
  GROUP BY event_slug, room
);

-- Backfill room ids on legacy rows by seeded room name.
UPDATE event_schedules
SET room_ids_json = COALESCE((
  SELECT json_array(r.id) FROM event_rooms r
  WHERE r.event_slug = event_schedules.event_slug AND r.name = event_schedules.room
), '[]');

-- Workspace rows for every event that already has schedule items or speakers.
INSERT OR IGNORE INTO event_schedule_workspace (event_slug, revision)
SELECT event_slug, 1 FROM (
  SELECT DISTINCT event_slug FROM event_schedules
  UNION
  SELECT DISTINCT event_slug FROM event_speakers
);
