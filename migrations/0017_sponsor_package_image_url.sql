-- Add configurable placement image URL to the sponsor_packages table.
ALTER TABLE sponsor_packages
  ADD COLUMN image_url TEXT;

-- Seed default placement visual URLs for standard Community Day 2026 packages
UPDATE sponsor_packages SET image_url = 'https://avatars.awscommunity.id/comday-26/website.png' WHERE event_slug = 'community-day-2026' AND id = 'web-logo' AND image_url IS NULL;
UPDATE sponsor_packages SET image_url = 'https://avatars.awscommunity.id/comday-26/social-media.png' WHERE event_slug = 'community-day-2026' AND id = 'social-blast' AND image_url IS NULL;
UPDATE sponsor_packages SET image_url = 'https://avatars.awscommunity.id/comday-26/video-ad.png' WHERE event_slug = 'community-day-2026' AND id = 'video-ad' AND image_url IS NULL;
UPDATE sponsor_packages SET image_url = 'https://avatars.awscommunity.id/comday-26/website.png' WHERE event_slug = 'community-day-2026' AND id = 'email-footer' AND image_url IS NULL;
UPDATE sponsor_packages SET image_url = 'https://avatars.awscommunity.id/comday-26/t-shirt.png' WHERE event_slug = 'community-day-2026' AND id = 'tshirt' AND image_url IS NULL;
UPDATE sponsor_packages SET image_url = 'https://avatars.awscommunity.id/comday-26/lanyard.png' WHERE event_slug = 'community-day-2026' AND id = 'lanyard' AND image_url IS NULL;
UPDATE sponsor_packages SET image_url = 'https://avatars.awscommunity.id/comday-26/backdrop.png' WHERE event_slug = 'community-day-2026' AND id = 'backdrop' AND image_url IS NULL;
UPDATE sponsor_packages SET image_url = 'https://avatars.awscommunity.id/comday-26/booth.png' WHERE event_slug = 'community-day-2026' AND id = 'mc-mention' AND image_url IS NULL;
