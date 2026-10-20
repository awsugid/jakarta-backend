-- Add configurable deadline date (e.g. print/media submission cutoff) to sponsor_packages table.
ALTER TABLE sponsor_packages
  ADD COLUMN deadline_date TEXT;

-- Seed default print deadline date for Community Day 2026 print media packages
UPDATE sponsor_packages SET deadline_date = '2026-10-25' WHERE event_slug = 'community-day-2026' AND id IN ('tshirt', 'lanyard', 'backdrop') AND deadline_date IS NULL;
