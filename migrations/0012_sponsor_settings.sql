-- Sponsor settings table for event-level configuration (e.g., USD exchange rate).
CREATE TABLE IF NOT EXISTS sponsor_settings (
  event_slug TEXT PRIMARY KEY,
  usd_exchange_rate INTEGER NOT NULL DEFAULT 17000 CHECK (usd_exchange_rate >= 1000 AND usd_exchange_rate <= 1000000),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO sponsor_settings (event_slug, usd_exchange_rate)
VALUES ('community-day-2026', 17000)
ON CONFLICT(event_slug) DO NOTHING;
