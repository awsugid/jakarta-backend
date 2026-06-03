-- Application response index for fast lookups
-- D1 is used as a fast lookup/index layer. FormBricks remains the source of truth for full response data.

CREATE TABLE IF NOT EXISTS application_response_index (
  id TEXT PRIMARY KEY,
  form_id TEXT NOT NULL REFERENCES application_forms(id),
  formbricks_survey_id TEXT NOT NULL,
  formbricks_response_id TEXT NOT NULL,
  normalized_email TEXT NOT NULL,
  normalized_linkedin_url TEXT,
  finished INTEGER NOT NULL DEFAULT 1,
  status TEXT NOT NULL DEFAULT 'active',
  submitted_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(form_id, formbricks_response_id)
);

CREATE INDEX IF NOT EXISTS idx_response_index_email
  ON application_response_index(normalized_email, status);

CREATE INDEX IF NOT EXISTS idx_response_index_form_email
  ON application_response_index(form_id, normalized_email, status);

CREATE INDEX IF NOT EXISTS idx_response_index_form_linkedin
  ON application_response_index(form_id, normalized_linkedin_url, status);
