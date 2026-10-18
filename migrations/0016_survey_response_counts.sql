-- Cache of per-survey Formbricks response counts backing GET /api/admin/forms
-- and the PUT form toggle. Counts come from FULL live upstream walks (finished
-- + in-progress), never the partial D1 response index.
--
-- Freshness: a row is fresh while refreshed_at >= datetime('now','-300 seconds')
-- (evaluated SQL-side). Fresh rows serve with zero upstream calls; expired or
-- missing rows trigger exactly one synchronous full walk per distinct survey.
-- A failed refresh keeps the stale row (or leaves the row absent -> API null)
-- and never writes a count, so no misleading 0 and no freshness advance.
CREATE TABLE IF NOT EXISTS survey_response_counts (
  survey_id TEXT PRIMARY KEY,
  response_count INTEGER NOT NULL CHECK (response_count >= 0),
  refreshed_at TEXT NOT NULL DEFAULT (datetime('now'))
);
