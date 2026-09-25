-- Custom tag assignments on Formbricks responses (app-owned, survey-scoped).
-- Catalog is derived: distinct assigned labels per survey. No rename/global-delete.
-- Assignments are independent of application_response_index: unfinished or
-- unindexed responses can carry tags too.

CREATE TABLE IF NOT EXISTS response_tags (
  survey_id TEXT NOT NULL,
  response_id TEXT NOT NULL,
  tag TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (survey_id, response_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_response_tags_survey_tag
  ON response_tags(survey_id, tag);
