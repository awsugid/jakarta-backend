-- Session metadata from Sessionize categories: spoken languages and audience
-- level per schedule item. Stored as JSON string arrays like the other list
-- columns; NOT NULL '[]' keeps rows without metadata cheap and simple.

ALTER TABLE event_schedules ADD COLUMN languages_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE event_schedules ADD COLUMN levels_json TEXT NOT NULL DEFAULT '[]';

-- One-time backfill for sessions imported before this migration: languages and
-- levels are re-derived from the cached Sessionize source payload (only
-- accepted sessions live in the cache). Matching is by session_id and
-- case-insensitive category NAME — no hard-coded language/level values.
-- Safety:
-- - only rows whose array is still '[]' are touched, so manual admin edits and
--   re-runs never overwrite anything;
-- - rows without a Sessionize match (manually seeded, no session_id) keep '[]';
-- - the publication snapshot is NOT mutated — the next publish picks the
--   backfilled values up;
-- - the UPDATE fires the 0029 workspace-revision triggers, invalidating any
--   outstanding review/publish tokens (fail-closed, by design).
UPDATE event_schedules AS s
SET languages_json = COALESCE((
  SELECT json_group_array(v) FROM (
    SELECT DISTINCT json_extract(ci.value, '$.name') AS v
    FROM sessionize_source_cache c,
         json_each(json_extract(c.payload, '$.sessions')) cs,
         json_each(json_extract(cs.value, '$.categories')) cat,
         json_each(json_extract(cat.value, '$.categoryItems')) ci
    WHERE c.id = 1
      AND json_extract(cs.value, '$.id') = s.session_id
      AND lower(trim(json_extract(cat.value, '$.name'))) IN ('language', 'languages')
      AND trim(coalesce(json_extract(ci.value, '$.name'), '')) <> ''
  )
), '[]')
WHERE s.session_id IS NOT NULL
  AND s.languages_json = '[]';

UPDATE event_schedules AS s
SET levels_json = COALESCE((
  SELECT json_group_array(v) FROM (
    SELECT DISTINCT json_extract(ci.value, '$.name') AS v
    FROM sessionize_source_cache c,
         json_each(json_extract(c.payload, '$.sessions')) cs,
         json_each(json_extract(cs.value, '$.categories')) cat,
         json_each(json_extract(cat.value, '$.categoryItems')) ci
    WHERE c.id = 1
      AND json_extract(cs.value, '$.id') = s.session_id
      AND lower(trim(json_extract(cat.value, '$.name'))) IN ('level', 'session level', 'experience level', 'audience level')
      AND trim(coalesce(json_extract(ci.value, '$.name'), '')) <> ''
  )
), '[]')
WHERE s.session_id IS NOT NULL
  AND s.levels_json = '[]';
