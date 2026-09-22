-- Optional manual USD price override per sponsor package.
-- NULL = derive from the event's usd_exchange_rate estimate; a set value is
-- used verbatim (e.g. converted $88 rounded by the admin to $100).
ALTER TABLE sponsor_packages
  ADD COLUMN price_usd REAL CHECK (price_usd IS NULL OR (price_usd > 0 AND price_usd <= 1000000));
