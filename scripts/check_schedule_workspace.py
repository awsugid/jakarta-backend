#!/usr/bin/env python3
"""Regression check for the schedule workspace (rooms, formats, revision
triggers, atomic publish guard) against a real in-memory SQLite DB.

Applies the real migrations (0019..0029) and executes SQL statements extracted
directly from ScheduleRepository with EXACT text and bind order — no
placeholder rewriting — so drift between Rust code and this check fails
loudly. Stdlib only, no new dependencies.

Run: python3 scripts/check_schedule_workspace.py
"""
import json
import re
import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MIGRATIONS = ROOT / "migrations"
SCHEDULE_RS = ROOT / "src" / "schedule" / "repository.rs"


def extract_sql(rust_path: Path, var: str) -> str:
    """Pull a `let <var> = "...";` SQL literal from Rust source."""
    src = rust_path.read_text()
    m = re.search(rf"let {var}\s*=\s*(.*?);", src, re.DOTALL)
    assert m, f"{var} not found in {rust_path.name}"
    lit = m.group(1)
    lit = lit.replace('r#"', "").replace('"#', "")
    lit = re.sub(r"\\\n\s*", " ", lit)  # Rust string line continuations
    lit = re.sub(r'"\s*"', " ", lit)  # adjacent literal concatenation
    lit = lit.replace('"', "")
    return re.sub(r"\s+", " ", lit).strip()


def apply_migration(conn: sqlite3.Connection, filename: str) -> None:
    conn.executescript((MIGRATIONS / filename).read_text())


def main() -> None:
    # SQL pulled live from the repository — never edit these constants by hand.
    create_sql = extract_sql(SCHEDULE_RS, "create_sql")
    update_sql = extract_sql(SCHEDULE_RS, "update_sql")
    publish_guard_sql = extract_sql(SCHEDULE_RS, "publish_guard_sql")
    room_occupancy_sql = extract_sql(SCHEDULE_RS, "room_occupancy_sql")
    format_occupancy_sql = extract_sql(SCHEDULE_RS, "format_occupancy_sql")
    room_update_sql = extract_sql(SCHEDULE_RS, "room_update_sql")
    insert_sql = extract_sql(SCHEDULE_RS, "insert_sql")

    # Sessionize imports must ignore upstream scheduling entirely.
    assert "NULL, NULL, NULL, NULL" in insert_sql, (
        "sessionize import SQL must NULL out room/track/starts/ends"
    )
    assert re.search(r"\b0, \?, \?, '\[\]', 0, \?\)", insert_sql), (
        "sessionize import SQL must insert is_active=0, empty rooms, no speaker ids"
    )
    # Publish guard: numbered params must match the 3 binds the Rust code sends
    # (slug, snapshot, expected revision).
    assert publish_guard_sql.count("?1") == 2 and publish_guard_sql.count("?3") == 1, (
        "publish_guard_sql must use ?1 (slug, twice) and ?3 (expected revision)"
    )
    assert "?4" not in publish_guard_sql, "publish_guard_sql must not reference ?4"

    conn = sqlite3.connect(":memory:")
    # Upgrade path: legacy tables first, seed rows, THEN the workspace
    # migrations — exactly what a production D1 upgrade sees.
    for mig in [
        "0019_event_speakers.sql",
        "0022_event_schedules.sql",
        "0023_sessionize_identity.sql",
        "0025_schedule_speakers_json.sql",
    ]:
        apply_migration(conn, mig)

    def rev():
        return conn.execute(
            "SELECT revision FROM event_schedule_workspace WHERE event_slug = ?", (slug,)
        ).fetchone()[0]

    slug = "community-days-2026"
    conn.execute(
        "INSERT INTO event_schedules (id, event_slug, title, room, starts_at, ends_at,"
        " session_type, is_service_session, is_active, display_order, speakers_json)"
        " VALUES ('sched-1', ?, 'Keynote', 'Main Hall', '2026-10-21T09:00:00Z',"
        " '2026-10-21T10:00:00Z', 'Plenum / Keynote', 0, 1, 0,"
        " ?)",
        (slug, json.dumps([{"id": "spk-1", "name": "Ada"}])),
    )
    conn.execute(
        "INSERT INTO event_schedules (id, event_slug, title, room, session_type,"
        " is_service_session, is_active, display_order)"
        " VALUES ('sched-2', ?, 'Coffee Break', 'Workshop Room', 'Service Session', 1, 1, 1)",
        (slug,),
    )
    conn.execute(
        "INSERT INTO event_schedules (id, event_slug, title, session_type,"
        " is_service_session, is_active, display_order)"
        " VALUES ('sched-3', ?, 'Imported draft', 'Technical Session', 0, 0, 2)",
        (slug,),
    )
    conn.commit()

    # --- 0026/0028: rooms + formats seeded from legacy names/labels.
    apply_migration(conn, "0026_schedule_workspace.sql")
    apply_migration(conn, "0028_talk_formats.sql")

    rooms = conn.execute(
        "SELECT id, name FROM event_rooms WHERE event_slug = ? ORDER BY display_order", (slug,)
    ).fetchall()
    assert [r[1] for r in rooms] == ["Main Hall", "Workshop Room"], rooms
    main_id, workshop_id = rooms[0][0], rooms[1][0]

    room_ids = conn.execute(
        "SELECT room_ids_json FROM event_schedules WHERE id = 'sched-1'"
    ).fetchone()[0]
    assert json.loads(room_ids) == [main_id], room_ids

    formats = conn.execute(
        "SELECT id, name FROM event_formats WHERE event_slug = ? ORDER BY display_order", (slug,)
    ).fetchall()
    assert [f[1] for f in formats] == [
        "Plenum / Keynote",
        "Service Session",
        "Technical Session",
    ], formats
    fmt_id = formats[0][0]
    assert (
        conn.execute("SELECT format_id FROM event_schedules WHERE id = 'sched-1'").fetchone()[0]
        == fmt_id
    )

    ws = conn.execute(
        "SELECT revision FROM event_schedule_workspace WHERE event_slug = ?", (slug,)
    ).fetchone()
    assert ws and ws[0] == 1, ws

    # --- 0027 backfill: publication snapshot from CURRENT ACTIVE schedule only.
    apply_migration(conn, "0027_publications_backfill.sql")
    pub = conn.execute(
        "SELECT snapshot_json, published_at FROM event_schedule_publications WHERE event_slug = ?",
        (slug,),
    ).fetchone()
    assert pub, "backfilled publication row expected"
    doc = json.loads(pub[0])
    ids = [i["id"] for i in doc["schedule"]]
    assert ids == ["sched-1", "sched-2"], ids  # draft sched-3 excluded
    item = doc["schedule"][0]
    assert item["speakers"] == [{"id": "spk-1", "name": "Ada"}]
    assert item["rooms"] == ["Main Hall"], item["rooms"]
    assert item["room"] == "Main Hall"
    assert item["roomIds"] == [main_id]
    assert item["isServiceSession"] == 0 and item["isActive"] == 1, item

    # --- 0029 triggers: every relevant mutation bumps the workspace revision
    # atomically (single statement). Apply AFTER capturing revision 1 above.
    apply_migration(conn, "0029_workspace_revision_triggers.sql")

    # item INSERT bumps
    conn.execute(
        create_sql,
        (
            "sched-new",  # id
            slug,  # event_slug
            None,  # session_id
            "New Talk",  # title
            None,  # description
            None,  # speaker_id
            None,  # speaker_name
            None,  # speaker_role
            None,  # speaker_company
            None,  # speaker_avatar
            None,  # speaker_social_url
            None,  # room (legacy label)
            None,  # track
            None,  # starts_at
            None,  # ends_at
            "Technical Session",  # session_type
            0,  # is_service_session
            1,  # is_active
            0,  # display_order
            "[]",  # speakers_json
            json.dumps([main_id]),  # room_ids_json
            0,  # all_rooms
            "[]",  # speaker_ids_json
            None,  # format_id
        ),
    )
    assert rev() == 2, "item INSERT must bump workspace revision via trigger"
    row = conn.execute(
        "SELECT revision, room_ids_json, all_rooms, speaker_ids_json, format_id"
        " FROM event_schedules WHERE id = 'sched-new'"
    ).fetchone()
    assert row[0] == 1 and json.loads(row[1]) == [main_id] and row[2] == 0, row

    # --- Item update CAS: correct revision bumps item+workspace, stale is a no-op.
    conn.execute(
        update_sql,
        (
            None,  # session_id
            "Renamed",  # title
            None,  # description
            None,  # speaker_id
            None,  # speaker_name
            None,  # speaker_role
            None,  # speaker_company
            None,  # speaker_avatar
            None,  # speaker_social_url
            None,  # speakers_json (keep)
            None,  # speaker_ids_json (keep)
            None,  # room label
            None,  # track
            None,  # starts_at
            None,  # ends_at
            "Technical Session",  # session_type
            None,  # format_id (keep)
            json.dumps([workshop_id]),  # room_ids_json
            0,  # all_rooms
            0,  # is_service_session
            1,  # is_active
            0,  # display_order
            slug,
            "sched-new",
            1,  # revision
        ),
    )
    renamed = conn.execute(
        "SELECT title, revision, room_ids_json FROM event_schedules WHERE id = 'sched-new'"
    ).fetchone()
    assert renamed[0] == "Renamed" and renamed[1] == 2 and json.loads(renamed[2]) == [
        workshop_id
    ], renamed
    assert rev() == 3, "item UPDATE must bump workspace revision via trigger"

    # Empty-string starts_at is normalized to None by the Rust layer
    # (clearable/normalize_clear, unit-tested in repository.rs) before this SQL
    # runs; the bind below is exactly what the Rust code sends for a clear.
    clear_vals = [None] * 25
    clear_vals[1] = "Renamed"
    clear_vals[13] = None  # starts_at cleared (Rust normalized "" -> None)
    clear_vals[17] = json.dumps([workshop_id])
    clear_vals[18] = 0
    clear_vals[19] = 0
    clear_vals[20] = 1
    clear_vals[21] = 0
    clear_vals[22] = slug
    clear_vals[23] = "sched-new"
    clear_vals[24] = 2
    conn.execute(update_sql, tuple(clear_vals))
    assert (
        conn.execute(
            "SELECT starts_at FROM event_schedules WHERE id = 'sched-new'"
        ).fetchone()[0]
        is None
    ), "empty-string starts_at must clear the column"

    stale_vals = [None] * 25
    stale_vals[1] = "Stale"
    stale_vals[17] = "[]"
    stale_vals[18] = 0
    stale_vals[19] = 0
    stale_vals[20] = 1
    stale_vals[21] = 0
    stale_vals[22] = slug
    stale_vals[23] = "sched-new"
    stale_vals[24] = 1  # stale revision
    conn.execute(update_sql, tuple(stale_vals))
    still = conn.execute(
        "SELECT title, revision FROM event_schedules WHERE id = 'sched-new'"
    ).fetchone()
    assert still[0] == "Renamed", still  # stale CAS changed nothing

    # item DELETE bumps
    conn.execute(
        "DELETE FROM event_schedules WHERE event_slug = ? AND id = ? AND revision = ?",
        (slug, "sched-new", 3),
    )
    assert rev() >= 5, "item DELETE must bump workspace revision via trigger"

    # room + speaker-directory mutations bump too
    before = rev()
    conn.execute(room_update_sql, ("Great Hall", 0, 0, slug, main_id, 1))
    assert rev() == before + 1, "room UPDATE must bump workspace revision via trigger"
    before = rev()
    conn.execute(
        "INSERT INTO event_speakers (event_slug, id, name, role, photo_url, talk_title,"
        " display_order) VALUES (?, 'spk-9', 'Cara', 'DevRel', 'p', 'T', 9)",
        (slug,),
    )
    assert rev() == before + 1, "speaker INSERT must bump workspace revision via trigger"

    # --- Publish guard: EXACT extracted SQL and bind order (slug, snapshot,
    # expected revision). Stale token violates NOT NULL and writes nothing.
    current = rev()
    snapshot = json.dumps({"schedule": []})
    try:
        conn.execute(publish_guard_sql, (slug, snapshot, current + 5))
    except sqlite3.IntegrityError:
        pass
    else:
        raise AssertionError("stale publish revision must violate NOT NULL")
    assert (
        conn.execute(
            "SELECT snapshot_json FROM event_schedule_publications WHERE event_slug = ?",
            (slug,),
        ).fetchone()[0]
        != snapshot
    ), "failed publish must not write"

    # Correct token publishes; the workspace bump (second batch statement)
    # is applied right after, exactly like the Rust db.batch.
    conn.execute(publish_guard_sql, (slug, snapshot, current))
    conn.execute(
        "UPDATE event_schedule_workspace SET revision = revision + 1 WHERE event_slug = ?",
        (slug,),
    )
    assert (
        conn.execute(
            "SELECT snapshot_json FROM event_schedule_publications WHERE event_slug = ?", (slug,)
        ).fetchone()[0]
        == snapshot
    )
    assert rev() == current + 1, "publish must consume its token"

    # Mutation after a review token was taken invalidates it (H2+M1 combined):
    data_taken_at = rev()
    conn.execute(room_update_sql, ("Grand Hall", 0, 0, slug, main_id, 2))  # mutation lands
    try:
        conn.execute(publish_guard_sql, (slug, json.dumps({"schedule": []}), data_taken_at))
    except sqlite3.IntegrityError:
        pass
    else:
        raise AssertionError("publish with a pre-mutation token must fail")

    # --- Room occupancy guard.
    occupants = conn.execute(room_occupancy_sql, (slug, main_id)).fetchall()
    assert [o[0] for o in occupants] == ["sched-1"], occupants
    fmt_occupants = conn.execute(format_occupancy_sql, (slug, fmt_id)).fetchall()
    assert [o[0] for o in fmt_occupants] == ["sched-1"], fmt_occupants

    print("check_schedule_workspace: OK")


if __name__ == "__main__":
    main()
