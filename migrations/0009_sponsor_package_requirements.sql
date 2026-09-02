-- Add configurable minimum-spend requirements and sponsor capacity to the
-- package table created by migration 0008. This forward migration is required
-- because 0008 may already be recorded as applied in existing D1 databases.
ALTER TABLE sponsor_packages
  ADD COLUMN minimum_spend_idr INTEGER
  CHECK (
    minimum_spend_idr IS NULL
    OR (minimum_spend_idr >= 1 AND minimum_spend_idr <= 1000000000)
  );

ALTER TABLE sponsor_packages
  ADD COLUMN max_sponsors INTEGER
  CHECK (
    max_sponsors IS NULL
    OR (max_sponsors >= 1 AND max_sponsors <= 10000)
  );

ALTER TABLE sponsor_packages
  ADD COLUMN reserved_sponsors INTEGER NOT NULL DEFAULT 0
  CHECK (
    reserved_sponsors >= 0
    AND reserved_sponsors <= 10000
    AND (max_sponsors IS NULL OR reserved_sponsors <= max_sponsors)
  );
