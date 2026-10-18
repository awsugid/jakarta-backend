-- Add topic_image_url column to event_speakers to show topic banner/cover image
ALTER TABLE event_speakers ADD COLUMN topic_image_url TEXT;

-- Seed topic images for Community Day 2026 speakers
UPDATE event_speakers
SET topic_image_url = 'https://images.unsplash.com/photo-1677442136019-21780efad99a?w=800&auto=format&fit=crop&q=80'
WHERE event_slug = 'community-day-2026' AND id = 'afif-akbar-iskandar';

UPDATE event_speakers
SET topic_image_url = 'https://images.unsplash.com/photo-1451187580459-43490279c0fa?w=800&auto=format&fit=crop&q=80'
WHERE event_slug = 'community-day-2026' AND id = 'petra-novandi-barus';

UPDATE event_speakers
SET topic_image_url = 'https://images.unsplash.com/photo-1563986768609-322da13575f3?w=800&auto=format&fit=crop&q=80'
WHERE event_slug = 'community-day-2026' AND id = 'deisya-nur-aisyah';

UPDATE event_speakers
SET topic_image_url = 'https://images.unsplash.com/photo-1667372393119-3d4c48d07fc9?w=800&auto=format&fit=crop&q=80'
WHERE event_slug = 'community-day-2026' AND id = 'muhammad-abdul-aziz';
