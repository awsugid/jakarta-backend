-- Add is_configurator_enabled column to sponsor_settings table.
-- 1 = Configurator is open and active for public enquiries (default).
-- 0 = Configurator is closed and hidden from public visitors.
ALTER TABLE sponsor_settings
ADD COLUMN is_configurator_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_configurator_enabled IN (0, 1));
