-- Customizable sponsor package groups. Groups are seeded per event and can be
-- renamed/reordered by admins; packages reference a group via group_id, which
-- supersedes the legacy category column for API/public grouping.
CREATE TABLE IF NOT EXISTS sponsor_package_groups (
  event_slug TEXT NOT NULL,
  id TEXT NOT NULL,
  label TEXT NOT NULL,
  display_order INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (event_slug, id),
  UNIQUE (event_slug, display_order)
);

-- Seed the two Community Day 2026 groups mirroring the legacy categories.
-- DO NOTHING keeps later admin rename/reorder edits when re-applied.
INSERT INTO sponsor_package_groups
  (event_slug, id, label, display_order)
VALUES
  ('community-day-2026', 'digital-media', 'Digital & Media', 1),
  ('community-day-2026', 'onsite-physical', 'On-Site & Physical', 2)
ON CONFLICT(event_slug, id) DO NOTHING;

-- Packages point at a group; NULL = ungrouped (legacy rows only).
-- No CHECK on display_order: the two-phase negative-order swap used by the
-- admin update writes temporary negative values inside one D1 batch.
ALTER TABLE sponsor_packages ADD COLUMN group_id TEXT;

-- Backfill existing packages from the legacy category. Scoped to the event
-- whose groups are seeded above; other events keep NULL until they get groups.
UPDATE sponsor_packages
SET group_id = CASE category
  WHEN 'digital' THEN 'digital-media'
  WHEN 'onsite' THEN 'onsite-physical'
END
WHERE event_slug = 'community-day-2026';
