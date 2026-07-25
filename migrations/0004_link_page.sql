-- Linktree-style customizable link page.
-- Singleton community page profile plus ordered link items.
CREATE TABLE IF NOT EXISTS link_page (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  title TEXT NOT NULL,
  bio TEXT,
  avatar_url TEXT,
  background TEXT NOT NULL,
  button_style TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS link_items (
  id TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  url TEXT NOT NULL,
  icon TEXT,
  is_enabled INTEGER NOT NULL CHECK (is_enabled IN (0, 1)),
  display_order INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Seed singleton page with community defaults.
INSERT INTO link_page (id, title, bio, avatar_url, background, button_style)
VALUES (
  1,
  'AWS User Group Jakarta',
  'Jakarta community of AWS users and cloud builders. Join our meetups, talks, and Community Day events.',
  NULL,
  'dark',
  'solid'
)
ON CONFLICT(id) DO NOTHING;
