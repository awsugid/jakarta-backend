-- Sponsor packages for event-scoped sponsorship configuration.
-- Definitions/order are seeded read-only; admin initially mutates price_idr
-- and is_unlocked. Additional unlock/capacity fields are added by migration 0009.
CREATE TABLE IF NOT EXISTS sponsor_packages (
  event_slug TEXT NOT NULL,
  id TEXT NOT NULL,
  name TEXT NOT NULL,
  advantage TEXT NOT NULL,
  category TEXT NOT NULL CHECK (category IN ('digital', 'onsite')),
  price_idr INTEGER NOT NULL CHECK (price_idr > 0 AND price_idr <= 1000000000),
  is_unlocked INTEGER NOT NULL DEFAULT 1 CHECK (is_unlocked IN (0, 1)),
  display_order INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (event_slug, id),
  UNIQUE (event_slug, display_order)
);

-- Seed the eight Community Day 2026 packages from the website config, all
-- unlocked. DO NOTHING keeps later admin price/unlock edits when re-applied.
INSERT INTO sponsor_packages
  (event_slug, id, name, advantage, category, price_idr, display_order)
VALUES
  (
    'community-day-2026',
    'web-logo',
    'Website Logo',
    'High-intent brand exposure on jakarta.awscommunity.id',
    'digital',
    2500000,
    1
  ),
  (
    'community-day-2026',
    'social-blast',
    'Social Blast',
    'Direct amplification of products or hiring to digital community',
    'digital',
    2500000,
    2
  ),
  (
    'community-day-2026',
    'video-ad',
    'Video Ad',
    '30–60 second narrative slot or platform demo during breaks',
    'digital',
    5000000,
    3
  ),
  (
    'community-day-2026',
    'email-footer',
    'Email Footer',
    'Brand placement on ticket confirmations, logistics, and post-event email',
    'digital',
    8000000,
    4
  ),
  (
    'community-day-2026',
    'tshirt',
    'T-Shirt',
    'Long-tail visual marketing through event merchandise',
    'onsite',
    6000000,
    5
  ),
  (
    'community-day-2026',
    'lanyard',
    'Lanyard',
    'Eye-level presence worn by participants',
    'onsite',
    7500000,
    6
  ),
  (
    'community-day-2026',
    'backdrop',
    'Backdrop',
    'Branding in official and participant event photos',
    'onsite',
    4000000,
    7
  ),
  (
    'community-day-2026',
    'mc-mention',
    'MC Mention',
    'Verbal sponsor callouts during breaks',
    'onsite',
    3500000,
    8
  )
ON CONFLICT(event_slug, id) DO NOTHING;
