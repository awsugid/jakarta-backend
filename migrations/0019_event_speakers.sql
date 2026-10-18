-- Event speakers table for configuring featured speakers.
-- Stores speaker details, photo URL, company, talk title, talk format, bio,
-- social/LinkedIn URL, display order, and active/inactive status.
CREATE TABLE IF NOT EXISTS event_speakers (
  event_slug TEXT NOT NULL,
  id TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT NOT NULL,
  company TEXT,
  photo_url TEXT NOT NULL,
  talk_title TEXT NOT NULL,
  talk_format TEXT DEFAULT 'Technical Session',
  bio TEXT,
  social_url TEXT,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  display_order INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (event_slug, id),
  UNIQUE (event_slug, display_order)
);

