ALTER TABLE profiles ADD COLUMN username TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_profiles_username ON profiles(username) WHERE username IS NOT NULL;
