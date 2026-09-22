-- Optional manual USD threshold override per sponsor tier.
-- NULL = derive from the event's usd_exchange_rate estimate; a set value is
-- used verbatim against USD-denominated sponsor totals. Same shape/bounds
-- as sponsor_packages.price_usd (0013).
ALTER TABLE sponsor_tiers
  ADD COLUMN threshold_usd REAL CHECK (threshold_usd IS NULL OR (threshold_usd > 0 AND threshold_usd <= 1000000));
