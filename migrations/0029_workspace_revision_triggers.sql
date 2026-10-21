-- Native triggers keep the event workspace revision in lockstep with every
-- mutation relevant to publication (schedule items, rooms, talk formats,
-- speaker directory). Single-statement atomicity closes the CAS race that
-- separate "mutate then bump" round-trips left open: a publish guard can no
-- longer observe post-mutation data with a pre-mutation revision.

CREATE TRIGGER IF NOT EXISTS trg_event_schedules_ws_ins AFTER INSERT ON event_schedules
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (NEW.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_schedules_ws_upd AFTER UPDATE ON event_schedules
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (NEW.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_schedules_ws_del AFTER DELETE ON event_schedules
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (OLD.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_rooms_ws_ins AFTER INSERT ON event_rooms
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (NEW.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_rooms_ws_upd AFTER UPDATE ON event_rooms
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (NEW.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_rooms_ws_del AFTER DELETE ON event_rooms
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (OLD.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_formats_ws_ins AFTER INSERT ON event_formats
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (NEW.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_formats_ws_upd AFTER UPDATE ON event_formats
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (NEW.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_formats_ws_del AFTER DELETE ON event_formats
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (OLD.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

-- Speaker directory rows freeze into schedule snapshots at publish time, so
-- directory mutations must invalidate review/publish tokens too.
CREATE TRIGGER IF NOT EXISTS trg_event_speakers_ws_ins AFTER INSERT ON event_speakers
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (NEW.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_speakers_ws_upd AFTER UPDATE ON event_speakers
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (NEW.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_event_speakers_ws_del AFTER DELETE ON event_speakers
BEGIN
  INSERT INTO event_schedule_workspace (event_slug, revision) VALUES (OLD.event_slug, 2)
  ON CONFLICT(event_slug) DO UPDATE SET revision = revision + 1;
END;
