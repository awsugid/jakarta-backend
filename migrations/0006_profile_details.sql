-- Additive self-service profile details (see Serena memory: email_keyed_self_service_profile_feature_plan).
-- Google snapshot upsert touches only name/picture/updated_at; editable fields below are user-owned.
ALTER TABLE profiles ADD COLUMN display_name TEXT;
ALTER TABLE profiles ADD COLUMN title TEXT;
ALTER TABLE profiles ADD COLUMN links_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE profiles ADD COLUMN is_public INTEGER NOT NULL DEFAULT 0;
ALTER TABLE profiles ADD COLUMN profile_updated_at TEXT;
