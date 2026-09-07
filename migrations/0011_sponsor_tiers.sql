-- Customizable sponsor tiers. Tiers were previously hard-coded in the
-- website (computeSponsorTier); storing them per event lets admins edit
-- labels, thresholds, and accents. Tier order IS threshold order (there is
-- deliberately no display_order); always list by threshold_idr DESC.
--
-- No CHECK on threshold_idr: the admin batch update rewrites thresholds
-- two-phase (temporary distinct negative values inside one D1 batch) so
-- swaps never collide with the immediate UNIQUE(event_slug, threshold_idr).
-- The valid 1..=1000000000 range is enforced at the application layer —
-- same trade-off as sponsor_package_groups.display_order in 0010.
CREATE TABLE IF NOT EXISTS sponsor_tiers (
  event_slug TEXT NOT NULL,
  id TEXT NOT NULL,
  label TEXT NOT NULL,
  threshold_idr INTEGER NOT NULL,
  accent TEXT NOT NULL DEFAULT 'default' CHECK (accent IN ('platinum','gold','silver','bronze','default')),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (event_slug, id),
  UNIQUE (event_slug, threshold_idr)
);

-- Seed Community Day 2026 tiers preserving the website's hard-coded
-- computeSponsorTier behavior exactly; threshold 1 replaces the old
-- `total > 0` branch for Community Supporter. DO NOTHING keeps later
-- admin edits when the migration is re-applied.
INSERT INTO sponsor_tiers
  (event_slug, id, label, threshold_idr, accent)
VALUES
  ('community-day-2026', 'platinum', 'Platinum', 40000000, 'platinum'),
  ('community-day-2026', 'gold', 'Gold', 25000000, 'gold'),
  ('community-day-2026', 'silver', 'Silver', 10000000, 'silver'),
  ('community-day-2026', 'supporter', 'Community Supporter', 1, 'default')
ON CONFLICT(event_slug, id) DO NOTHING;
