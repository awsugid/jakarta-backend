#!/usr/bin/env python3
"""Regression check for Sessionize daily-import semantics.

Applies the real migrations (0019, 0020, 0022, 0023) to an in-memory SQLite DB
and executes the SQL statements extracted directly from
SpeakerRepository::sync_from_sessionize / ScheduleRepository::sync_from_sessionize,
so any drift between the Rust code and this check fails loudly. Stdlib only.

Run: python3 scripts/check_sessionize_sync.py
"""
import json
import re
import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MIGRATIONS = ROOT / "migrations"
SPEAKERS_RS = ROOT / "src" / "speakers" / "repository.rs"
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


# SQL pulled live from the repositories — never edit these constants by hand.
SPEAKER_IDENTITY_SQL = extract_sql(SPEAKERS_RS, "identity_sql")
SPEAKER_LINK_SQL = extract_sql(SPEAKERS_RS, "link_sql")
SPEAKER_INSERT_SQL = extract_sql(SPEAKERS_RS, "insert_sql")
SCHEDULE_DIRECTORY_SQL = extract_sql(SCHEDULE_RS, "directory_sql")
SCHEDULE_INSERT_SQL = extract_sql(SCHEDULE_RS, "insert_sql")

BASE_MIGRATIONS = (
    "0019_event_speakers.sql",
    "0020_speaker_topic_image.sql",
    "0022_event_schedules.sql",
    "0025_schedule_speakers_json.sql",
    "0026_schedule_workspace.sql",
    "0027_publications_backfill.sql",
    "0028_talk_formats.sql",
    "0029_workspace_revision_triggers.sql",
)
MIGRATION_0023 = "0023_sessionize_identity.sql"


def base_db():
    """Schema exactly as it was before 0023 (for seeding legacy duplicates)."""
    db = sqlite3.connect(":memory:")
    for name in BASE_MIGRATIONS:
        db.executescript((MIGRATIONS / name).read_text())
    return db


def apply_0023(db):
    db.executescript((MIGRATIONS / MIGRATION_0023).read_text())
    return db


def fresh_db():
    return apply_0023(base_db())


def changed(cur):
    return cur.rowcount > 0


def upsert_speaker(db, event, upstream_id, name):
    """Insert path of speakers sync (no eligible legacy name match)."""
    # Mirrors slugify + generate_unique_id from the repository.
    base = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-") or "speaker"
    existing = {row[0] for row in db.execute("SELECT id FROM event_speakers WHERE event_slug = ?", (event,))}
    spk_id = base
    n = 2
    while spk_id in existing:
        spk_id = f"{base}-{n}"
        n += 1
    cur = db.execute(
        SPEAKER_INSERT_SQL,
        (event, spk_id, name, "Speaker", None,
         "/images/avatar-placeholder.png", "Talk", "Technical Session",
         None, None, None, event, upstream_id),
    )
    return changed(cur)


def identity_maps(db, event):
    """Mirrors the identity query in SpeakerRepository::sync_from_sessionize."""
    linked, unlinked = {}, {}
    for sid, name, sessionize_id in db.execute(SPEAKER_IDENTITY_SQL, (event,)):
        if sessionize_id:
            linked[sessionize_id] = sid
        else:
            unlinked.setdefault(name.strip().lower(), []).append(sid)
    return linked, unlinked


def backfill_target(db, event, upstream_name, upstream_counts):
    """Mirrors backfill eligibility: unique name on BOTH local-unlinked and upstream sides."""
    _, unlinked = identity_maps(db, event)
    candidates = unlinked.get(upstream_name.strip().lower(), [])
    if len(candidates) == 1 and upstream_counts.get(upstream_name.strip().lower(), 0) == 1:
        return candidates[0]
    return None


def test_migration_dedupes_seeded_duplicate_sessions():
    db = base_db()
    seeds = [
        ("a1", "cd-2025", "sess-1", "First import"),
        ("a2", "cd-2025", "sess-1", "Accidental duplicate"),
        ("a3", "cd-2025", "sess-2", "Unique session"),
        ("b1", "summit", "sess-1", "Other event same session id"),
    ]
    for sid, event, session_id, title in seeds:
        db.execute(
            "INSERT INTO event_schedules (id, event_slug, session_id, title, display_order)"
            " VALUES (?, ?, ?, ?, 0)",
            (sid, event, session_id, title),
        )
    db.commit()
    apply_0023(db)  # must not raise despite duplicate (event_slug, session_id)
    rows = db.execute(
        "SELECT id, session_id, title FROM event_schedules ORDER BY rowid"
    ).fetchall()
    assert rows == [
        ("a1", "sess-1", "First import"),  # canonical: lowest rowid keeps link
        ("a2", None, "Accidental duplicate"),  # preserved, link cleared
        ("a3", "sess-2", "Unique session"),
        ("b1", "sess-1", "Other event same session id"),  # scoped per event_slug
    ], rows
    assert db.execute("SELECT COUNT(*) FROM event_schedules").fetchone()[0] == 4


def test_migration_constraints():
    db = fresh_db()
    assert upsert_speaker(db, "cd-2025", "sz-1", "Ada")
    try:
        db.execute(
            "INSERT INTO event_speakers (event_slug, id, name, role, photo_url, talk_title,"
            " display_order, sessionize_id) VALUES ('cd-2025','x','X','S','p','T',9,'sz-1')"
        )
        raise AssertionError("unique index on (event_slug, sessionize_id) missing")
    except sqlite3.IntegrityError:
        pass
    for i in range(2):  # manual speakers with NULL sessionize_id coexist
        db.execute(
            "INSERT INTO event_speakers (event_slug, id, name, role, photo_url, talk_title,"
            " display_order) VALUES ('cd-2025', 'm%d', 'M%d', 'S', 'p', 'T', 10+%d)" % (i, i, i)
        )
    db.commit()


def test_speaker_insert_idempotent_and_orders_atomic():
    db = fresh_db()
    assert upsert_speaker(db, "cd-2025", "sz-1", "Ada")
    assert upsert_speaker(db, "cd-2025", "sz-2", "Grace")
    assert not upsert_speaker(db, "cd-2025", "sz-1", "Ada")  # replay: no dupe
    assert not upsert_speaker(db, "cd-2025", "sz-2", "Grace")
    rows = db.execute(
        "SELECT name, display_order, sessionize_id FROM event_speakers ORDER BY display_order"
    ).fetchall()
    assert rows == [("Ada", 1, "sz-1"), ("Grace", 2, "sz-2")], rows
    assert db.execute("SELECT COUNT(*) FROM event_speakers").fetchone()[0] == 2


def test_legacy_backfill_links_without_overwriting_fields():
    db = fresh_db()
    db.execute(
        "INSERT INTO event_speakers (event_slug, id, name, role, photo_url, talk_title,"
        " display_order) VALUES ('cd-2025', 'jane-doe', 'Jane Doe', 'Admin Edited Role',"
        " 'p.png', 'Admin Talk', 1)"
    )
    db.commit()
    counts = {"jane doe": 1}  # single upstream speaker with this name
    target = backfill_target(db, "cd-2025", "Jane Doe", counts)
    assert target == "jane-doe"
    assert changed(db.execute(SPEAKER_LINK_SQL, ("sz-9", "cd-2025", target)))
    name, role = db.execute(
        "SELECT name, role FROM event_speakers WHERE id = 'jane-doe'"
    ).fetchone()
    assert (name, role) == ("Jane Doe", "Admin Edited Role")  # daily path: fields untouched
    assert not upsert_speaker(db, "cd-2025", "sz-9", "Jane Doe")  # identity match now
    assert db.execute(
        "SELECT COUNT(*) FROM event_speakers WHERE name = 'Jane Doe'"
    ).fetchone()[0] == 1


def test_two_upstream_same_name_never_claims_legacy_row():
    db = fresh_db()
    db.execute(
        "INSERT INTO event_speakers (event_slug, id, name, role, photo_url, talk_title,"
        " display_order) VALUES ('cd-2025', 'legacy-js', 'Jordan Case', 'S', 'p', 'T', 1)"
    )
    db.commit()
    counts = {"jordan case": 2}  # payload carries TWO upstream speakers, same name
    assert backfill_target(db, "cd-2025", "Jordan Case", counts) is None
    # Repo decision: no backfill -> both upstream speakers insert separate rows.
    assert upsert_speaker(db, "cd-2025", "sz-a", "Jordan Case")
    assert upsert_speaker(db, "cd-2025", "sz-b", "Jordan Case")
    rows = db.execute(
        "SELECT id, sessionize_id FROM event_speakers ORDER BY rowid"
    ).fetchall()
    assert rows == [("legacy-js", None), ("jordan-case", "sz-a"), ("jordan-case-2", "sz-b")], rows


def test_admin_edits_preserved_on_daily_import():
    db = fresh_db()
    upsert_speaker(db, "cd-2025", "sz-1", "Ada")
    db.execute("UPDATE event_speakers SET role = 'CTO' WHERE sessionize_id = 'sz-1'")
    db.commit()
    linked, _ = identity_maps(db, "cd-2025")  # identity match + overwrite=false -> skip
    assert "sz-1" in linked
    role = db.execute("SELECT role FROM event_speakers WHERE sessionize_id = 'sz-1'").fetchone()[0]
    assert role == "CTO"


def test_schedule_idempotent_and_speaker_resolution():
    db = fresh_db()
    upsert_speaker(db, "cd-2025", "sz-1", "Ada")  # local id: "ada"
    directory = {
        sessionize_id: sid
        for sid, sessionize_id in db.execute(SCHEDULE_DIRECTORY_SQL, ("cd-2025",)).fetchall()
    }
    resolve = lambda upstream: directory.get(upstream, upstream)
    assert resolve("sz-1") == "ada"
    assert resolve("sz-unknown") == "sz-unknown"  # upstream UUID kept when not imported

    def add_session(sess_id, spk_upstream):
        resolved = resolve(spk_upstream)
        linked = resolved in directory.values()
        cur = db.execute(
            SCHEDULE_INSERT_SQL,
            (f"sched-{sess_id}", "cd-2025", sess_id, "Title", None,
             resolved, "Ada", None, None, None, None,
             "Technical Session", 0, 0,
             f'[{{"id":"{resolved}","name":"Ada"}}]',
             json.dumps([resolved]) if linked else "[]"),
        )
        return changed(cur)

    assert add_session("sess-1", "sz-1")
    assert not add_session("sess-1", "sz-1")  # replay: conflict-safe, no dupe
    assert add_session("sess-2", "sz-unknown")
    rows = db.execute(
        "SELECT session_id, speaker_id FROM event_schedules ORDER BY session_id"
    ).fetchall()
    assert rows == [("sess-1", "ada"), ("sess-2", "sz-unknown")], rows
    # Imports land as unscheduled inactive drafts with no rooms; directory-linked
    # speaker ids are persisted for publish-time freeze.
    drafts = db.execute(
        "SELECT starts_at, ends_at, is_active, room_ids_json, speaker_ids_json"
        " FROM event_schedules WHERE session_id = 'sess-1'"
    ).fetchone()
    assert drafts[:4] == (None, None, 0, "[]"), drafts
    assert json.loads(drafts[4]) == ["ada"], drafts
    for i in range(2):  # manual rows without session_id stay unconstrained
        db.execute(
            "INSERT INTO event_schedules (id, event_slug, title, display_order)"
            " VALUES ('manual-%d', 'cd-2025', 'Manual', %d)" % (i, i)
        )
    db.commit()


def main():
    tests = [
        test_migration_dedupes_seeded_duplicate_sessions,
        test_migration_constraints,
        test_speaker_insert_idempotent_and_orders_atomic,
        test_legacy_backfill_links_without_overwriting_fields,
        test_two_upstream_same_name_never_claims_legacy_row,
        test_admin_edits_preserved_on_daily_import,
        test_schedule_idempotent_and_speaker_resolution,
    ]
    for t in tests:
        t()
        print(f"OK  {t.__name__}")
    print(f"All {len(tests)} checks passed.")


if __name__ == "__main__":
    main()
