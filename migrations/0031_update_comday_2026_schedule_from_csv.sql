-- 0031_update_comday_2026_schedule_from_csv.sql
-- Update Community Day 2026 schedule from Master Sheet Comday 2026 - Rundown Schedule.csv

-- Ensure event_schedule_workspace exists for community-day-2026
INSERT OR IGNORE INTO event_schedule_workspace (event_slug, revision) VALUES ('community-day-2026', 1);

-- 1. Upsert all 49 rundown entries for community-day-2026
INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-01', 'community-day-2026', NULL, 'Open Gate', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T08:00:00+07:00', '2026-10-31T09:00:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 1, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-90975f21fb668ff1', 'community-day-2026', NULL, 'Opening', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T09:00:00+07:00', '2026-10-31T09:05:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 2, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-03', 'community-day-2026', NULL, 'Welcoming Speech - AWS User Group Jakarta', NULL,
  NULL, 'Suhri', 'Organizer', 'Organizer', NULL, NULL,
  '[{"name": "Suhri", "role": "Organizer", "company": "Organizer"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T09:05:00+07:00', '2026-10-31T09:10:00+07:00', 'Keynote', 'fmt-05d2e30ff02ca579', '[]', '[]',
  0, 1, 3, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-04', 'community-day-2026', NULL, 'Welcoming Speech - AWS Representative', NULL,
  NULL, 'Eryan Ariowibowo', 'AWS', 'AWS', NULL, NULL,
  '[{"name": "Eryan Ariowibowo", "role": "AWS", "company": "AWS"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T09:10:00+07:00', '2026-10-31T09:15:00+07:00', 'Keynote', 'fmt-05d2e30ff02ca579', '[]', '[]',
  0, 1, 4, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-05', 'community-day-2026', NULL, 'Welcoming Speech - Binus Representative', NULL,
  NULL, 'Fabian Surya Pramudya, S.T., M.T., Ph.D.', 'Binus', 'Binus', NULL, NULL,
  '[{"name": "Fabian Surya Pramudya, S.T., M.T., Ph.D.", "role": "Binus", "company": "Binus"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T09:15:00+07:00', '2026-10-31T09:25:00+07:00', 'Keynote', 'fmt-05d2e30ff02ca579', '[]', '[]',
  0, 1, 5, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-21f74127b5ea3e9f', 'community-day-2026', NULL, 'Context Intelligence for your AI Agent at Scale', NULL,
  NULL, 'Juan Hasang', 'AWS', 'AWS', NULL, NULL,
  '[{"name": "Juan Hasang", "role": "AWS", "company": "AWS"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T09:25:00+07:00', '2026-10-31T10:05:00+07:00', 'Keynote', 'fmt-05d2e30ff02ca579', '["English", "Bahasa Indonesia"]', '["Intermediate"]',
  0, 1, 6, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-ad9fe5fa8b336a6b', 'community-day-2026', NULL, 'Building Trusted AI: Peluang Karier Masa Depan, Belajar dari Industri Perbankan', 'Saat AI makin pintar, skill apa yang makin dicari? Belajar dari industri perbankan, sesi ini membahas cara membangun AI yang bisa dipercaya, mulai dari security, data privacy, dan agentic AI sampai Post-Quantum Cryptography, serta peluang karier yang muncul dari setiap topik.',
  'ricky', 'Ricky', 'Sr. Solutions Architect', 'AWS', 'https://avatars.awscommunity.id/community-day-2026/speakers/73a06b647a30_1793298475.jpg', NULL,
  '[{"name": "Ricky", "role": "Sr. Solutions Architect", "company": "AWS", "avatar": "https://avatars.awscommunity.id/community-day-2026/speakers/73a06b647a30_1793298475.jpg"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '["ricky"]', 0,
  '2026-10-31T10:05:00+07:00', '2026-10-31T10:45:00+07:00', 'Keynote', 'fmt-05d2e30ff02ca579', '[]', '["Beginner"]',
  0, 1, 7, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-08', 'community-day-2026', NULL, 'Documentation', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T10:45:00+07:00', '2026-10-31T10:50:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 8, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-09', 'community-day-2026', NULL, 'Ice Breaking: Kahoot', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T10:50:00+07:00', '2026-10-31T10:55:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 9, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-10', 'community-day-2026', NULL, 'Split Stage', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T10:55:00+07:00', '2026-10-31T11:00:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 10, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-5c25dbfab6f73114', 'community-day-2026', NULL, 'When AI Agents Touch Production: Making Them Safe, Observable and Recoverable on AWS', 'In this session, we will see an AI agent investigates a live production break on your phone. A person approves the fix, and it''s rolled back inside 10 minutes. Through one AWS architecture (AgentCore, IAM, S3, CloudWatch), we answer three questions every team should ask before an agent gets credentials: can it touch my data, what did it actually do, and can we undo it? You leave with a minimal reference pattern, real config, and three questions for your team on Monday.',
  NULL, 'Steve Ng', 'Sponsor', 'Sponsor', NULL, NULL,
  '[{"name": "Steve Ng", "role": "Sponsor", "company": "Sponsor"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T11:00:00+07:00', '2026-10-31T11:40:00+07:00', 'Technical Talk', 'fmt-f1cff0bf9873baad', '["English"]', '["Intermediate"]',
  0, 1, 11, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-82d02b3754b9d99e', 'community-day-2026', NULL, 'From AI Foundations to Cloud Deployment: Bridging Theory and Practice with AWS in Higher Education', NULL,
  NULL, 'Lili Wulandhari', 'Sponsor', 'Sponsor', NULL, NULL,
  '[{"name": "Lili Wulandhari", "role": "Sponsor", "company": "Sponsor"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T11:40:00+07:00', '2026-10-31T12:20:00+07:00', 'Technical Talk', 'fmt-f1cff0bf9873baad', '["Bahasa Indonesia"]', '["Beginner"]',
  0, 1, 12, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-d82323cad426562a', 'community-day-2026', NULL, 'Prayer, Networking, & Lunch Time', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T12:20:00+07:00', '2026-10-31T13:20:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 13, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-3788070924251249', 'community-day-2026', '1319894', 'AWS DevOps Agent: Beyond the Runbook', 'DevOps talk',
  'romar-cablao', 'Romar Cablao', 'DevOps', 'Ascendia Works', 'https://avatars.awscommunity.id/community-day-2026/speakers/c4ed7f42f1bf_1792584844.png', 'https://linkedin.com/in/romarcablao',
  '[{"name": "Romar Cablao", "role": "DevOps", "company": "Ascendia Works", "avatar": "https://avatars.awscommunity.id/community-day-2026/speakers/c4ed7f42f1bf_1792584844.png", "socialUrl": "https://linkedin.com/in/romarcablao"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '["romar-cablao"]', 0,
  '2026-10-31T13:20:00+07:00', '2026-10-31T13:55:00+07:00', 'Technical Talk', 'fmt-f1cff0bf9873baad', '["English"]', '["Intermediate"]',
  0, 1, 14, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-9223083017052656', 'community-day-2026', '1342026', 'Zero Trust for AI: Securing Bedrock, SageMaker, and Agentic Workloads', 'Zero trust talk',
  'nagababu-medicharla', 'Nagababu Medicharla', 'Senior Manager', 'Searce', 'https://cdn.sessionize.com/image/ff08-0o0o0-RuigjeVkuPLWVszN71u9hn.jpg?download=nagababu-medicharla.jpg', 'https://www.linkedin.com/in/nagababu-medicharla-b2a91a117/',
  '[{"name": "Nagababu Medicharla", "role": "Senior Manager", "company": "Searce", "avatar": "https://cdn.sessionize.com/image/ff08-0o0o0-RuigjeVkuPLWVszN71u9hn.jpg?download=nagababu-medicharla.jpg", "socialUrl": "https://www.linkedin.com/in/nagababu-medicharla-b2a91a117/"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '["nagababu-medicharla"]', 0,
  '2026-10-31T13:55:00+07:00', '2026-10-31T14:30:00+07:00', 'Technical Talk', 'fmt-f1cff0bf9873baad', '["English"]', '["Intermediate"]',
  0, 1, 15, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-845054287768393', 'community-day-2026', '1329523', 'Running Untrusted Code: From LeetCode to AI Agents, on Lambda MicroVMs', 'MicroVMs talk',
  'srinu-madhav-vysyaraju', 'Srinu Madhav Vysyaraju', 'Backend Developer', 'sls.guru', 'https://cdn.sessionize.com/image/e659-400o400o1-XNE5NHBECtC1gu89Zv8twE.jpg', 'https://linkedin.com/in/srinumadhav',
  '[{"name": "Srinu Madhav Vysyaraju", "role": "Backend Developer", "company": "sls.guru", "avatar": "https://cdn.sessionize.com/image/e659-400o400o1-XNE5NHBECtC1gu89Zv8twE.jpg", "socialUrl": "https://linkedin.com/in/srinumadhav"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '["srinu-madhav-vysyaraju"]', 0,
  '2026-10-31T14:30:00+07:00', '2026-10-31T15:05:00+07:00', 'Technical Talk', 'fmt-f1cff0bf9873baad', '["English"]', '["Beginner"]',
  0, 1, 16, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-07ab528f275712af', 'community-day-2026', NULL, 'Over-Provisioned to Optimized: Autonomous K8s Right-Sizing with ML', 'K8s talk',
  NULL, 'Doddy Kristianto', 'CFP', 'CFP', NULL, NULL,
  '[{"name": "Doddy Kristianto", "role": "CFP", "company": "CFP"}, {"name": "Nasrullah Pandu Dewantara", "role": "CFP", "company": "CFP"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T15:05:00+07:00', '2026-10-31T15:40:00+07:00', 'Technical Talk', 'fmt-f1cff0bf9873baad', '["Bahasa Indonesia"]', '["Intermediate"]',
  0, 1, 17, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-248243db5568ff4a', 'community-day-2026', NULL, 'Ask Me Anything: Real Talk from Real Builders, Cloud, AI, & Career', NULL,
  NULL, 'Adrianus Kurnadi', 'CFP & AWS', 'CFP & AWS', NULL, NULL,
  '[{"name": "Adrianus Kurnadi", "role": "CFP & AWS", "company": "CFP & AWS"}, {"name": "Arief Nugraha", "role": "CFP & AWS", "company": "CFP & AWS"}, {"name": "Ricky", "role": "CFP & AWS", "company": "CFP & AWS"}, {"name": "Shah Hassan", "role": "CFP & AWS", "company": "CFP & AWS"}, {"name": "Doddy Kristianto", "role": "CFP & AWS", "company": "CFP & AWS"}]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T15:40:00+07:00', '2026-10-31T16:20:00+07:00', 'Fireside Chat', 'fmt-4fb3702d62656b53', '["English", "Bahasa Indonesia"]', '["Beginner", "Intermediate", "Advanced"]',
  0, 1, 18, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-19', 'community-day-2026', NULL, 'Ice Breaking: Kahoot', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T16:20:00+07:00', '2026-10-31T16:30:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 19, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-20', 'community-day-2026', NULL, 'Feedback Form', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T16:30:00+07:00', '2026-10-31T16:35:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 20, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-21', 'community-day-2026', NULL, 'Documentation', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T16:35:00+07:00', '2026-10-31T16:40:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 21, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-93c6f99c2d2d5916', 'community-day-2026', NULL, 'Closing', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T16:40:00+07:00', '2026-10-31T16:45:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 22, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-23', 'community-day-2026', NULL, 'Networking', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Aula 800', 'Main Stage', '["room-0e34023b76bf185f"]', '[]', 0,
  '2026-10-31T16:45:00+07:00', '2026-10-31T17:00:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 23, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-24', 'community-day-2026', NULL, 'Opening', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '[]', 0,
  '2026-10-31T11:00:00+07:00', '2026-10-31T11:10:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 24, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-9944626715470675', 'community-day-2026', '1341923', 'Defense in Depth: Benchmarking DoS Attack Resilience on Public vs. Private AWS VPC Subnets', 'Many engineering teams default to deploying EC2 instances directly in public subnets because it is quick, convenient, and cost-effective, often unaware of the severe performance and availability risks when under attack. While architectural isolation is one of the most effective first lines of defense in cloud infrastructure, the real-world trade-offs are rarely benchmarked side-by-side. Grounded in the Network Development Life Cycle (NDLC) framework, this session presents an empirical benchmark and real-world evaluation of DoS resilience within AWS Virtual Private Cloud (VPC), comparing direct Public Subnet exposure against an ALB-fronted Private Subnet architecture across Layer 3, 4, and 7 attack vectors (ICMP, SYN, and HTTP Floods). Attendees will gain data-driven insights into how network isolation preserves low latency and CPU stability under heavy traffic, alongside actionable architectural patterns for hardening AWS workloads without sacrificing operational simplicity.',
  'sabrina-zulfa-wahidah', 'Sabrina Zulfa Wahidah', 'DevOps', 'HOSTi | AWS Community Builder', 'https://cdn.sessionize.com/image/91a5-400o400o1-H2wZq1GXzkjebK1uPXpe8x.jpg', 'https://www.linkedin.com/in/sabrina-zulfa',
  '[{"name": "Sabrina Zulfa Wahidah", "role": "DevOps", "company": "HOSTi | AWS Community Builder", "avatar": "https://cdn.sessionize.com/image/91a5-400o400o1-H2wZq1GXzkjebK1uPXpe8x.jpg", "socialUrl": "https://www.linkedin.com/in/sabrina-zulfa"}]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '["sabrina-zulfa-wahidah"]', 0,
  '2026-10-31T11:10:00+07:00', '2026-10-31T11:45:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["Bahasa Indonesia"]', '["Intermediate"]',
  0, 1, 25, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-4e6d1360987407ce', 'community-day-2026', NULL, 'The Customer Paid Once. Why Did We Process It Twice?', NULL,
  'shah-hassan', 'Shah Hassan', 'Cloud Engineer', 'NDeS', 'https://avatars.awscommunity.id/community-day-2026/speakers/bc8cd0e6c1ca_1792584889.jpg', 'https://www.linkedin.com/in/tsshahhassan',
  '[{"name": "Shah Hassan", "role": "Cloud Engineer", "company": "NDeS", "avatar": "https://avatars.awscommunity.id/community-day-2026/speakers/bc8cd0e6c1ca_1792584889.jpg", "socialUrl": "https://www.linkedin.com/in/tsshahhassan"}]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '["shah-hassan"]', 0,
  '2026-10-31T11:45:00+07:00', '2026-10-31T12:20:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["English"]', '["Intermediate"]',
  0, 1, 26, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-3e9bed9760d0dcfa', 'community-day-2026', NULL, 'Prayer, Networking, & Lunch Time', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '[]', 0,
  '2026-10-31T12:20:00+07:00', '2026-10-31T13:20:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 27, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-4482659081859841', 'community-day-2026', '1340724', 'Defense-in-Depth di FSI: DMZ & Security Layer pada AWS Landing Zone Multi-Account', 'FSI talk',
  'wisnugara-wisnugara', 'Wisnugara Wisnugara', 'SRE', 'BTPN Syariah', 'https://avatars.awscommunity.id/community-day-2026/speakers/8b678b7b9e31_1792584696.jpg', 'https://linkedin.com.com/in/wisnugara',
  '[{"name": "Wisnugara Wisnugara", "role": "SRE", "company": "BTPN Syariah", "avatar": "https://avatars.awscommunity.id/community-day-2026/speakers/8b678b7b9e31_1792584696.jpg", "socialUrl": "https://linkedin.com.com/in/wisnugara"}]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '["wisnugara-wisnugara"]', 0,
  '2026-10-31T13:20:00+07:00', '2026-10-31T13:55:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["Bahasa Indonesia"]', '["Intermediate"]',
  0, 1, 28, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-8177173290213899', 'community-day-2026', '1341454', 'Preventive Cloud Governance: Enforcing AWS Guardrails with Terraform and OPA', 'Governance talk',
  'muhammad-arif-wicaksana', 'Muhammad Arif Wicaksana', 'Resident Solutions Engineer', 'IBM', 'https://cdn.sessionize.com/image/3229-400o400o1-Xz7YBVwGeLthhS97Ksouoo.jpg', 'https://www.linkedin.com/in/muhammad-arif-wicaksana/',
  '[{"name": "Muhammad Arif Wicaksana", "role": "Resident Solutions Engineer", "company": "IBM", "avatar": "https://cdn.sessionize.com/image/3229-400o400o1-Xz7YBVwGeLthhS97Ksouoo.jpg", "socialUrl": "https://www.linkedin.com/in/muhammad-arif-wicaksana/"}]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '["muhammad-arif-wicaksana"]', 0,
  '2026-10-31T13:55:00+07:00', '2026-10-31T14:30:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["Bahasa Indonesia", "English"]', '["Beginner"]',
  0, 1, 29, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-4dabdb3280647485', 'community-day-2026', NULL, 'Beyond the Console: Building Customized Cloud Cost Estimators with AWS SDK', 'SDK talk',
  NULL, 'Ikhsan Agil Kusuma', 'CFP', 'CFP', NULL, NULL,
  '[{"name": "Ikhsan Agil Kusuma", "role": "CFP", "company": "CFP"}]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '[]', 0,
  '2026-10-31T14:30:00+07:00', '2026-10-31T15:05:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["Bahasa Indonesia"]', '["Beginner"]',
  0, 1, 30, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-7239408f8b497445', 'community-day-2026', NULL, 'One Pipeline, Many AWS Accounts: Centralized CI/CD with Cross-Account Roles', 'CI/CD talk',
  NULL, 'Ahmad Nurhidayat', 'CFP', 'CFP', NULL, NULL,
  '[{"name": "Ahmad Nurhidayat", "role": "CFP", "company": "CFP"}]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '[]', 0,
  '2026-10-31T15:05:00+07:00', '2026-10-31T15:40:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '[]', '["Intermediate"]',
  0, 1, 31, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-32', 'community-day-2026', NULL, 'Documentation', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '[]', 0,
  '2026-10-31T15:40:00+07:00', '2026-10-31T15:45:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 32, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-33', 'community-day-2026', NULL, 'Merge Stage', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 804', 'Classroom 2', '["room-994b49f25b203ad3"]', '[]', 0,
  '2026-10-31T15:45:00+07:00', '2026-10-31T15:50:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 33, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-34', 'community-day-2026', NULL, 'Opening', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '[]', 0,
  '2026-10-31T11:00:00+07:00', '2026-10-31T11:10:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 34, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-8901585139301958', 'community-day-2026', '1337894', 'Open Source Tools that Helped Us on Journey of Serve, Scale, Fine-Tune, and Survive LLMs on EKS', 'Talk ini membahas perjalanan saya membangun infrastruktur LLM end-to-end di atas Amazon EKS, mulai dari model serving dan distributed inference hingga GPU autoscaling dan fine-tuning. Semuanya dibangun dari nol menggunakan berbagai tools open source.',
  'daniel-pepuho', 'Daniel Pepuho', 'Infra Engineer', 'AWS Community Builder', 'https://cdn.sessionize.com/image/b6cf-400o400o1-pwTATc3c6Er7uVMQXSvBWA.jpg', 'https://www.linkedin.com/in/daniel-pepuho',
  '[{"name": "Daniel Pepuho", "role": "Infra Engineer", "company": "AWS Community Builder", "avatar": "https://cdn.sessionize.com/image/b6cf-400o400o1-pwTATc3c6Er7uVMQXSvBWA.jpg", "socialUrl": "https://www.linkedin.com/in/daniel-pepuho"}, {"name": "Nur Muhammad Ainul Yaqin", "role": "Solution Architect", "company": "PT Astragraphia Information Technology (AGIT)", "avatar": "https://cdn.sessionize.com/image/8ac8-400o400o1-BGWENUevissx2cTGUqawMB.jpg", "socialUrl": "https://www.linkedin.com/in/masnurm"}]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '["daniel-pepuho", "nur-muhammad-ainul-yaqin"]', 0,
  '2026-10-31T11:10:00+07:00', '2026-10-31T11:45:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["Bahasa Indonesia"]', '["Intermediate"]',
  0, 1, 35, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-5938844021818', 'community-day-2026', '1318832', 'From MongoDB 3.0 on EC2 to 8.0 on EKS - A Migration That Failed, Then Succeeded', 'Migration talk',
  'muhammad-bagus-ramadhan', 'Muhammad Bagus Ramadhan', 'SRE', 'Kloia', 'https://cdn.sessionize.com/image/dc5a-400o400o1-fDhHzHPayUMgUrp7KeHuf3.png', 'https://linkedin.com/in/odhonbagus',
  '[{"name": "Muhammad Bagus Ramadhan", "role": "SRE", "company": "Kloia", "avatar": "https://cdn.sessionize.com/image/dc5a-400o400o1-fDhHzHPayUMgUrp7KeHuf3.png", "socialUrl": "https://linkedin.com/in/odhonbagus"}]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '["muhammad-bagus-ramadhan"]', 0,
  '2026-10-31T11:45:00+07:00', '2026-10-31T12:20:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["Bahasa Indonesia", "English"]', '["Intermediate"]',
  0, 1, 36, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-eb07d2b73de22ca0', 'community-day-2026', NULL, 'Prayer, Networking, & Lunch Time', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '[]', 0,
  '2026-10-31T12:20:00+07:00', '2026-10-31T13:20:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 37, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-01569854690268424', 'community-day-2026', '1319233', 'The Pragmatic Data Lake: Decoupling Compute & Storage on S3', 'Data lake talk',
  'guntur-dwi-cahyono', 'Guntur Dwi Cahyono', 'Data Engineer', 'Cloudmile', 'https://avatars.awscommunity.id/community-day-2026/speakers/2061cd7da7b3_1792584941.jpg', 'https://www.linkedin.com/in/gunhachi',
  '[{"name": "Guntur Dwi Cahyono", "role": "Data Engineer", "company": "Cloudmile", "avatar": "https://avatars.awscommunity.id/community-day-2026/speakers/2061cd7da7b3_1792584941.jpg", "socialUrl": "https://www.linkedin.com/in/gunhachi"}]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '["guntur-dwi-cahyono"]', 0,
  '2026-10-31T13:20:00+07:00', '2026-10-31T13:55:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["Bahasa Indonesia"]', '["Intermediate"]',
  0, 1, 38, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-e8011564bcfeeac1', 'community-day-2026', NULL, 'Two Compute Models, One Edge: CloudFront Functions and Lambda@Edge', 'Edge talk',
  NULL, 'Islam Nurul Yakin', 'CFP', 'CFP', NULL, NULL,
  '[{"name": "Islam Nurul Yakin", "role": "CFP", "company": "CFP"}]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '[]', 0,
  '2026-10-31T13:55:00+07:00', '2026-10-31T14:30:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["English", "Bahasa Indonesia"]', '["Intermediate"]',
  0, 1, 39, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-af0ecc9ae28e8fa5', 'community-day-2026', NULL, 'Building a Real-Time Streaming Lakehouse on AWS: From Kinesis to Apache Iceberg via Managed Flink', 'Streaming talk',
  NULL, 'Muhammad Saipul Rohman', 'CFP', 'CFP', NULL, NULL,
  '[{"name": "Muhammad Saipul Rohman", "role": "CFP", "company": "CFP"}]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '[]', 0,
  '2026-10-31T14:30:00+07:00', '2026-10-31T15:05:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["Bahasa Indonesia", "English"]', '["Advanced"]',
  0, 1, 40, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'sched-6532048200957582', 'community-day-2026', '1343210', 'Stop Preparing for Audits: AWS Environment That Produces Evidence Continuously', 'Compliance talk',
  'yohana-gracia-naomi', 'Yohana Gracia Naomi', 'Solution Architect', 'Nauli Data', 'https://cdn.sessionize.com/image/af67-400o400o1-Na6vEXqSY81ajyNrRGzNVL.jpg', 'http://linkedin.com/in/yohanagntrp',
  '[{"name": "Yohana Gracia Naomi", "role": "Solution Architect", "company": "Nauli Data", "avatar": "https://cdn.sessionize.com/image/af67-400o400o1-Na6vEXqSY81ajyNrRGzNVL.jpg", "socialUrl": "http://linkedin.com/in/yohanagntrp"}]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '["yohana-gracia-naomi"]', 0,
  '2026-10-31T15:05:00+07:00', '2026-10-31T15:40:00+07:00', 'Lightning Talk', 'fmt-fb5f1f7fc82e1356', '["Bahasa Indonesia", "English"]', '["Advanced"]',
  0, 1, 41, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-42', 'community-day-2026', NULL, 'Documentation', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '[]', 0,
  '2026-10-31T15:40:00+07:00', '2026-10-31T15:45:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 42, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-43', 'community-day-2026', NULL, 'Merge Stage', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 806', 'Classroom 3', '["room-7348a1ffffdd5f3c"]', '[]', 0,
  '2026-10-31T15:45:00+07:00', '2026-10-31T15:50:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 43, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-44', 'community-day-2026', NULL, 'Opening', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 810', 'Classroom 1', '["room-a33d4792c0912b71"]', '[]', 0,
  '2026-10-31T11:00:00+07:00', '2026-10-31T11:05:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 44, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-39c9d66d25aa8433', 'community-day-2026', NULL, 'Architecting for the Future: Scaling Modern Workloads with AWS Cloud-Native Databases', NULL,
  'arief-nugraha', 'Arief Nugraha', 'Solution Architect', 'AWS', 'https://avatars.awscommunity.id/community-day-2026/speakers/d1c6e9d1e4a4_1792657447.jpg', NULL,
  '[{"name": "Arief Nugraha", "role": "Solution Architect", "company": "AWS", "avatar": "https://avatars.awscommunity.id/community-day-2026/speakers/d1c6e9d1e4a4_1792657447.jpg"}]', 'Class 810', 'Classroom 1', '["room-a33d4792c0912b71"]', '["arief-nugraha"]', 0,
  '2026-10-31T11:05:00+07:00', '2026-10-31T13:05:00+07:00', 'Workshop', 'fmt-a29210c07b1271dc', '["Bahasa Indonesia"]', '[]',
  0, 1, 45, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-b92c0721341cee5c', 'community-day-2026', NULL, 'Prayer, Networking, & Lunch Time', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 810', 'Classroom 1', '["room-a33d4792c0912b71"]', '[]', 0,
  '2026-10-31T13:05:00+07:00', '2026-10-31T14:05:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '["Bahasa Indonesia", "English"]', '[]',
  1, 1, 46, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-5a74849719628c44', 'community-day-2026', NULL, 'Build a Serverless AI Game with Amazon Rekognition and AWS Lambda', NULL,
  'felicia-suryawinata', 'Felicia Suryawinata', 'Associate Solutions Architect', 'AWS', 'https://avatars.awscommunity.id/community-day-2026/speakers/4d766360f27f_1792585355.jpg', 'https://www.linkedin.com/in/felicia-surya',
  '[{"name": "Felicia Suryawinata", "role": "Associate Solutions Architect", "company": "AWS", "avatar": "https://avatars.awscommunity.id/community-day-2026/speakers/4d766360f27f_1792585355.jpg", "socialUrl": "https://www.linkedin.com/in/felicia-surya"}]', 'Class 810', 'Classroom 1', '["room-a33d4792c0912b71"]', '["felicia-suryawinata"]', 0,
  '2026-10-31T14:05:00+07:00', '2026-10-31T16:05:00+07:00', 'Workshop', 'fmt-a29210c07b1271dc', '["Bahasa Indonesia", "English"]', '["Beginner"]',
  0, 1, 47, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-48', 'community-day-2026', NULL, 'Documentation', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 810', 'Classroom 1', '["room-a33d4792c0912b71"]', '[]', 0,
  '2026-10-31T16:05:00+07:00', '2026-10-31T16:10:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 48, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

INSERT INTO event_schedules (
  id, event_slug, session_id, title, description,
  speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url,
  speakers_json, room, track, room_ids_json, speaker_ids_json, all_rooms,
  starts_at, ends_at, session_type, format_id, languages_json, levels_json,
  is_service_session, is_active, display_order, revision, created_at, updated_at
) VALUES (
  'seed-comday26-49', 'community-day-2026', NULL, 'Merge Stage', NULL,
  NULL, NULL, NULL, NULL, NULL, NULL,
  '[]', 'Class 810', 'Classroom 1', '["room-a33d4792c0912b71"]', '[]', 0,
  '2026-10-31T16:10:00+07:00', '2026-10-31T16:20:00+07:00', 'Service Session', 'fmt-4f06792f048363d5', '[]', '[]',
  1, 1, 49, 1, datetime('now'), datetime('now')
) ON CONFLICT(id) DO UPDATE SET
  title = excluded.title,
  speaker_name = CASE WHEN excluded.speaker_name IS NOT NULL THEN excluded.speaker_name ELSE event_schedules.speaker_name END,
  speaker_role = CASE WHEN excluded.speaker_role IS NOT NULL THEN excluded.speaker_role ELSE event_schedules.speaker_role END,
  speaker_company = CASE WHEN excluded.speaker_company IS NOT NULL THEN excluded.speaker_company ELSE event_schedules.speaker_company END,
  speakers_json = CASE WHEN excluded.speakers_json IS NOT NULL AND excluded.speakers_json != '[]' THEN excluded.speakers_json ELSE event_schedules.speakers_json END,
  room = excluded.room,
  track = excluded.track,
  room_ids_json = excluded.room_ids_json,
  starts_at = excluded.starts_at,
  ends_at = excluded.ends_at,
  display_order = excluded.display_order,
  revision = event_schedules.revision + 1,
  updated_at = datetime('now');

-- 2. Update publication snapshot for community-day-2026
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
      'allRooms', json(CASE WHEN s.all_rooms = 1 THEN 'true' ELSE 'false' END),
      'track', s.track,
      'startsAt', s.starts_at,
      'endsAt', s.ends_at,
      'sessionType', s.session_type,
      'formatId', s.format_id,
      'format', f.name,
      'languages', json(s.languages_json),
      'levels', json(s.levels_json),
      'isServiceSession', json(CASE WHEN s.is_service_session = 1 THEN 'true' ELSE 'false' END),
      'isActive', json(CASE WHEN s.is_active = 1 THEN 'true' ELSE 'false' END),
      'displayOrder', s.display_order,
      'revision', s.revision,
      'createdAt', s.created_at,
      'updatedAt', s.updated_at
    ))
    FROM event_schedules s
    LEFT JOIN event_formats f ON f.id = s.format_id
    WHERE s.event_slug = e.event_slug AND s.is_active = 1
    ORDER BY s.starts_at ASC, s.display_order ASC
  ), json('[]'))),
  strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
  'migration_0031',
  w.revision
FROM (SELECT 'community-day-2026' AS event_slug) e
JOIN event_schedule_workspace w ON w.event_slug = e.event_slug
ON CONFLICT(event_slug) DO UPDATE SET
  snapshot_json = excluded.snapshot_json,
  published_at = excluded.published_at,
  published_by = excluded.published_by,
  revision = excluded.revision;
