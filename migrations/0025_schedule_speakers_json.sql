-- Multi-speaker sessions: speakers stored as a JSON array per schedule item.
-- Legacy singular speaker_* columns retained for compatibility; they mirror the
-- first entry of the array. Service sessions and speaker-less items store '[]'.
ALTER TABLE event_schedules ADD COLUMN speakers_json TEXT NOT NULL DEFAULT '[]';

-- Backfill from legacy singular fields. SQL NULLs become JSON nulls, which the
-- Rust ScheduleSpeaker deserializer drops (Option + skip_serializing_if).
UPDATE event_schedules SET speakers_json = CASE
  WHEN is_service_session = 1 OR speaker_name IS NULL OR trim(speaker_name) = ''
  THEN '[]'
  ELSE json_array(json_object(
    'id', speaker_id,
    'name', speaker_name,
    'role', speaker_role,
    'company', speaker_company,
    'avatar', speaker_avatar,
    'socialUrl', speaker_social_url
  ))
END;
