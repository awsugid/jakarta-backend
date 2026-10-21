-- D1 source cache for Sessionize upstream data.
-- Singleton row (id = 1): one Sessionize event per deployment, keyed by its
-- source URLs so URL changes invalidate the snapshot instead of serving stale data.
CREATE TABLE IF NOT EXISTS sessionize_source_cache (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  speakers_url TEXT NOT NULL,
  sessions_url TEXT NOT NULL,
  payload TEXT NOT NULL,
  refreshed_at TEXT NOT NULL DEFAULT (datetime('now'))
);
