-- Event sponsors table for configuring confirmed sponsor companies/partners.
-- Stores sponsor details, logo image URL, website URL, tier, nominal/price paid,
-- description/notes, display order, and active/inactive status.
CREATE TABLE IF NOT EXISTS event_sponsors (
  event_slug TEXT NOT NULL,
  id TEXT NOT NULL,
  name TEXT NOT NULL,
  logo_url TEXT NOT NULL,
  website_url TEXT,
  tier TEXT NOT NULL,
  price_idr INTEGER NOT NULL DEFAULT 0,
  description TEXT,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  display_order INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (event_slug, id),
  UNIQUE (event_slug, display_order)
);

-- Seed initial sponsor (BINUS University) for Community Day 2026
INSERT INTO event_sponsors
  (event_slug, id, name, logo_url, website_url, tier, price_idr, description, is_active, display_order)
VALUES
  (
    'community-day-2026',
    'binus-university',
    'BINUS University',
    '/assets/comday26/sponsors/binus.png',
    'https://binus.ac.id',
    'venue',
    0,
    'Official Venue Partner for Community Day 2026',
    1,
    1
  )
ON CONFLICT(event_slug, id) DO NOTHING;
