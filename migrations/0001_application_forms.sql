-- Application forms registry and policy store
-- D1 is used ONLY as form metadata/policy store, not for application response data.
-- FormBricks is the source of truth for responses.

CREATE TABLE IF NOT EXISTS application_forms (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('volunteer', 'speaker')),
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  formbricks_survey_id TEXT NOT NULL,
  formbricks_public_url TEXT,
  email_question_id TEXT NOT NULL,
  linkedin_question_id TEXT NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 1,
  opens_at TEXT,
  closes_at TEXT,
  editable_until TEXT,
  archive_after TEXT,
  display_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(kind, slug)
);

CREATE INDEX IF NOT EXISTS idx_application_forms_kind_active
  ON application_forms(kind, is_active, display_order);
