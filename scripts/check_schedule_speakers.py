#!/usr/bin/env python3
"""Regression check for multi-speaker schedule items (speakers_json).

Applies the real migrations (0019, 0022, 0023, 0025) to an in-memory SQLite DB
and executes the SQL statements extracted directly from
ScheduleRepository (create / update / sessionize import), so any drift between
the Rust code and this check fails loudly. Stdlib only.

Run: python3 scripts/check_schedule_speakers.py
"""
import json
import re
import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MIGRATIONS = ROOT / "migrations"
SCHEDULE_RS = ROOT / "src" / "schedule" / "repository.rs"

def extract_sql(rust_path: Path, var: str) -> str:
    """Pull a `let <var> = "...";` SQL literal (raw or backslash-continued) from Rust source."""
    src = rust_path.read_text()
    m = re.search(rf"let {var}\s*=\s*(.*?);", src, re.DOTALL)
    assert m, f"{var} not found in {rust_path.name}"
    lit = m.group(1)
    lit = lit.replace('r#"', "").replace('"#', "")
    lit = re.sub(r"\\\n\s*", " ", lit)  # Rust string line continuations
    lit = re.sub(r'"\s*"', " ", lit)  # adjacent literal concatenation
    lit = lit.replace('"', "")
    return re.sub(r"\s+", " ", lit).strip()

# SQL pulled live from the repository — never edit these constants by hand.
CREATE_SQL = extract_sql(SCHEDULE_RS, "create_sql")
UPDATE_SQL = extract_sql(SCHEDULE_RS, "update_sql")
DIRECTORY_SQL = extract_sql(SCHEDULE_RS, "directory_sql")
IMPORT_SQL = extract_sql(SCHEDULE_RS, "insert_sql")

PRE_0025 = ("0019_event_speakers.sql", "0022_event_schedules.sql", "0023_sessionize_identity.sql")
MIGRATION_0025 = "0025_schedule_speakers_json.sql"

def db_with(*names):
    db = sqlite3.connect(":memory:")
    for name in names:
        db.executescript((MIGRATIONS / name).read_text())
    return db

def fresh_db():
    return db_with(
        *PRE_0025,
        "0024_sessionize_cache.sql",
        MIGRATION_0025,
        "0026_schedule_workspace.sql",
        "0027_publications_backfill.sql",
        "0028_talk_formats.sql",
        "0029_workspace_revision_triggers.sql",
        "0030_schedule_languages_levels.sql",
    )

def run_create(db, item_id, event, speakers_json, **cols):
    """Mirror of create_schedule_item bind order."""
    cur = db.execute(
        CREATE_SQL,
        (
            item_id, event, cols.get("session_id"), cols.get("title", "T"), None,
            cols.get("speaker_id"), cols.get("speaker_name"), cols.get("speaker_role"),
            cols.get("speaker_company"), cols.get("speaker_avatar"), cols.get("speaker_social_url"),
            None, None, None, None, "Technical Session", 0, 1, 0,
            speakers_json,
            "[]", 0, "[]", None,
            "[]", "[]",  # languages_json, levels_json
        ),
    )
    db.commit()
    return cur

def run_update(db, event, item_id, speakers_json, title="T", speaker_name=None, revision=1):
    """Mirror of update_schedule_item bind order (speakers_json None = preserve)."""
    cur = db.execute(
        UPDATE_SQL,
        (
            None, title, None,
            None, speaker_name, None, None, None, None,
            speakers_json,
            None,
            None, None, None, None, "Technical Session", None,
            "[]", 0, 0, 1, 0,
            None, None,  # languages_json, levels_json (COALESCE keep)
            event, item_id, revision,
        ),
    )
    db.commit()
    return cur

def speakers_of(db, item_id):
    return json.loads(
        db.execute("SELECT speakers_json FROM event_schedules WHERE id = ?", (item_id,)).fetchone()[0]
    )

def test_migration_backfill_builds_array_from_singular_fields():
    db = db_with(*PRE_0025)  # schema before 0025: no speakers_json column yet
    db.execute(
        "INSERT INTO event_schedules (id, event_slug, session_id, title, speaker_id, speaker_name,"
        " speaker_role, speaker_company, speaker_avatar, speaker_social_url)"
        " VALUES ('a', 'cd-2025', 'sess-1', 'Talk', 'spk-1', 'Ada Lovelace', 'DevRel', 'AWS',"
        " 'https://img/ada.png', 'https://linkedin.com/in/ada')"
    )
    db.execute(  # service session with a contact name still imports as []
        "INSERT INTO event_schedules (id, event_slug, title, speaker_name, is_service_session)"
        " VALUES ('b', 'cd-2025', 'Registration', 'Front Desk', 1)"
    )
    db.execute(  # blank speaker name is "no speaker", not an empty speaker object
        "INSERT INTO event_schedules (id, event_slug, title, speaker_name)"
        " VALUES ('c', 'cd-2025', 'Open Space', '  ')"
    )
    db.commit()
    db.executescript((MIGRATIONS / MIGRATION_0025).read_text())

    assert speakers_of(db, "a") == [{
        "id": "spk-1", "name": "Ada Lovelace", "role": "DevRel", "company": "AWS",
        "avatar": "https://img/ada.png", "socialUrl": "https://linkedin.com/in/ada",
    }], speakers_of(db, "a")
    assert speakers_of(db, "b") == []
    assert speakers_of(db, "c") == []
    # rows inserted after the migration default to []
    db.execute("INSERT INTO event_schedules (id, event_slug, title) VALUES ('d', 'cd-2025', 'New')")
    db.commit()
    assert speakers_of(db, "d") == []

def test_create_two_speakers_roundtrip_and_singular_mirror():
    db = fresh_db()
    two = json.dumps([
        {"id": "ada", "name": "Ada Lovelace", "role": "DevRel", "socialUrl": "https://linkedin.com/in/ada"},
        {"id": "grace", "name": "Grace Hopper", "company": "US Navy"},
    ])
    run_create(db, "sched-1", "cd-2025", two, speaker_id="ada", speaker_name="Ada Lovelace",
               speaker_role="DevRel")
    assert speakers_of(db, "sched-1") == json.loads(two)
    row = db.execute(
        "SELECT speaker_id, speaker_name, speaker_role, speaker_company FROM event_schedules WHERE id = 'sched-1'"
    ).fetchone()
    assert row == ("ada", "Ada Lovelace", "DevRel", None), row  # singular mirrors FIRST speaker

    legacy = json.dumps([{"name": "Solo Sam"}])  # create with no speakers_json column value
    run_create(db, "sched-2", "cd-2025", legacy, speaker_name="Solo Sam")
    assert speakers_of(db, "sched-2") == [{"name": "Solo Sam"}]

def test_update_preserves_clears_and_legacy_path_keeps_array():
    db = fresh_db()
    two = json.dumps([{"id": "ada", "name": "Ada"}, {"id": "grace", "name": "Grace"}])
    run_create(db, "sched-1", "cd-2025", two, speaker_id="ada", speaker_name="Ada")

    # omitted speakers (None) -> COALESCE keeps array even on unrelated field edits
    run_update(db, "cd-2025", "sched-1", None, title="Renamed Talk", revision=1)
    assert speakers_of(db, "sched-1") == json.loads(two)
    title = db.execute("SELECT title FROM event_schedules WHERE id = 'sched-1'").fetchone()[0]
    assert title == "Renamed Talk"

    # legacy singular-only edit: array untouched (array is source of truth now)
    run_update(db, "cd-2025", "sched-1", None, speaker_name="Admin Edit", revision=2)
    assert speakers_of(db, "sched-1") == json.loads(two)
    name = db.execute("SELECT speaker_name FROM event_schedules WHERE id = 'sched-1'").fetchone()[0]
    assert name == "Admin Edit"

    # explicit [] clears array AND singular columns
    run_update(db, "cd-2025", "sched-1", "[]", revision=3)
    row = db.execute(
        "SELECT speakers_json, speaker_id, speaker_name FROM event_schedules WHERE id = 'sched-1'"
    ).fetchone()
    assert row == ("[]", None, None), row

def test_import_all_speakers_with_local_id_mapping_and_insert_only():
    db = fresh_db()
    for order, (sid, suid) in enumerate([("ada", "sz-1"), ("grace", "sz-2")]):
        db.execute(
            "INSERT INTO event_speakers (event_slug, id, name, role, photo_url, talk_title,"
            " display_order, sessionize_id) VALUES ('cd-2025', ?, ?, 'S', 'p', 'T', ?, ?)",
            (sid, sid.capitalize(), order, suid),
        )
    db.commit()
    # real directory query drives the id mapping, same as the repository
    directory = {suid: sid for sid, suid in db.execute(DIRECTORY_SQL, ("cd-2025",)).fetchall()}
    assert directory == {"sz-1": "ada", "sz-2": "grace"}

    two = json.dumps([
        {"id": directory["sz-1"], "name": "Ada", "avatar": "https://img/ada.png"},
        {"id": directory["sz-2"], "name": "Grace"},
    ])
    cur = db.execute(
        IMPORT_SQL,
        ("sched-s1", "cd-2025", "sess-1", "Joint Talk", None,
         "ada", "Ada", None, None, None, None,
         "Technical Session", 0, 0, two, '["ada","grace"]',
         '["English", "Bahasa Indonesia"]', "[]"),
    )
    assert cur.rowcount == 1
    assert speakers_of(db, "sched-s1") == json.loads(two)  # BOTH speakers stored
    assert db.execute(
        "SELECT speaker_id FROM event_schedules WHERE id = 'sched-s1'"
    ).fetchone()[0] == "ada"
    # session metadata extracted from categories persists on import
    assert json.loads(db.execute(
        "SELECT languages_json FROM event_schedules WHERE id = 'sched-s1'"
    ).fetchone()[0]) == ["English", "Bahasa Indonesia"]

    # replay: INSERT OR IGNORE is a no-op, admin edits to speakers survive (daily insert-only)
    db.execute(
        "UPDATE event_schedules SET speakers_json = ? WHERE id = 'sched-s1'",
        (json.dumps([{"name": "Admin Curated"}]),),
    )
    cur = db.execute(
        IMPORT_SQL,
        ("sched-s1-dup", "cd-2025", "sess-1", "Joint Talk", None,
         "ada", "Ada", None, None, None, None,
         "Technical Session", 0, 0, two, "[]", "[]", "[]"),
    )
    assert cur.rowcount == 0  # conflict on (event_slug, session_id) ignored
    assert speakers_of(db, "sched-s1") == [{"name": "Admin Curated"}]
    assert db.execute("SELECT COUNT(*) FROM event_schedules").fetchone()[0] == 1

    # service session import: speakers forced to [] even when upstream lists some
    db.execute(
        IMPORT_SQL,
        ("sched-s2", "cd-2025", "sess-2", "Lunch", None,
         None, None, None, None, None, None,
         "Service Session", 1, 0, "[]", "[]", "[]", "[]"),
    )
    db.commit()
    assert speakers_of(db, "sched-s2") == []

def main():
    tests = [
        test_migration_backfill_builds_array_from_singular_fields,
        test_create_two_speakers_roundtrip_and_singular_mirror,
        test_update_preserves_clears_and_legacy_path_keeps_array,
        test_import_all_speakers_with_local_id_mapping_and_insert_only,
    ]
    for t in tests:
        t()
        print(f"OK  {t.__name__}")
    print(f"All {len(tests)} checks passed.")

if __name__ == "__main__":
    main()
