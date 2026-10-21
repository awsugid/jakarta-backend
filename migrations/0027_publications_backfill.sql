-- Published schedule snapshots. One row per event, replaced atomically on
-- publish. Backfilled from the CURRENT ACTIVE schedule so the public endpoint
-- keeps serving the live schedule on upgrade instead of going blank.

CREATE TABLE IF NOT EXISTS event_schedule_publications (
  event_slug TEXT PRIMARY KEY,
  snapshot_json TEXT NOT NULL,
  published_at TEXT NOT NULL,
  published_by TEXT NOT NULL DEFAULT 'admin',
  revision INTEGER NOT NULL
);

INSERT INTO event_schedule_publications (event_slug, snapshot_json, published_at, published_by, revision)
SELECT e.event_slug,
  json_object('schedule', COALESCE((
    SELECT json_group_array(json_object(
      'id', s.id,
      'eventSlug', s.event_slug,
      'sessionId', s.session_id,
      'title', s.title,
      'description', s.description,
      'speakerId', s.speaker_id,
      'speakerName', s.speaker_name,
      'speakerRole', s.speaker_role,
      'speakerCompany', s.speaker_company,
      'speakerAvatar', s.speaker_avatar,
      'speakerSocialUrl', s.speaker_social_url,
      'speakers', json(s.speakers_json),
      'room', s.room,
      'rooms', CASE WHEN s.room IS NULL OR trim(s.room) = '' THEN json('[]') ELSE json_array(s.room) END,
      'roomIds', json(s.room_ids_json),
      'speakerIds', json(s.speaker_ids_json),
      'allRooms', s.all_rooms = 1,
      'track', s.track,
      'startsAt', s.starts_at,
      'endsAt', s.ends_at,
      'sessionType', s.session_type,
      'isServiceSession', s.is_service_session = 1,
      'isActive', s.is_active = 1,
      'displayOrder', s.display_order,
      'revision', s.revision,
      'createdAt', s.created_at,
      'updatedAt', s.updated_at
    ))
    FROM event_schedules s WHERE s.event_slug = e.event_slug AND s.is_active = 1
  ), json('[]'))),
  strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
  'migration',
  w.revision
FROM (SELECT DISTINCT event_slug FROM event_schedules WHERE is_active = 1) e
JOIN event_schedule_workspace w ON w.event_slug = e.event_slug;
