#!/usr/bin/env python3
"""Regression check for 0030 session metadata (languages/levels).

Applies the real migrations to an in-memory SQLite DB, seeds a
sessionize_source_cache payload shaped exactly like the Rust
SessionizeDataPayload (camelCase, only accepted sessions), then runs 0030 and
asserts the backfill contract:
- matching session_id rows get languages/levels from category NAMES
  (case-insensitive; no hard-coded values);
- rows with manual values or no Sessionize match keep what they have;
- no session_id (manually seeded CSV rows) -> stays [].

Run: python3 scripts/check_schedule_languages.py
"""
import json
import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MIGRATIONS = ROOT / "migrations"


def apply(conn, name):
    conn.executescript((MIGRATIONS / name).read_text())


def main():
    conn = sqlite3.connect(":memory:")
    apply(conn, "0022_event_schedules.sql")
    apply(conn, "0024_sessionize_cache.sql")

    # Cache payload mirrors SessionizeDataPayload serialization (camelCase).
    payload = {
        "speakers": [],
        "sessions": [
            {
                "id": "sz-1",
                "title": "Bilingual talk",
                "categories": [
                    {"id": 1, "name": "Language", "categoryItems": [
                        {"id": 11, "name": "English"},
                        {"id": 12, "name": "Bahasa Indonesia"},
                    ]},
                    {"id": 2, "name": "Job Title", "categoryItems": [
                        {"id": 21, "name": "Developer Advocate"},
                    ]},
                ],
            },
            {
                "id": "sz-2",
                "title": "Leveled talk",
                "categories": [
                    {"id": 3, "name": "Session Level", "categoryItems": [
                        {"id": 31, "name": "Advanced"},
                    ]},
                    {"id": 4, "name": "languages", "categoryItems": [
                        {"id": 41, "name": "English"},
                    ]},
                ],
            },
            {  # accepted session with no metadata categories at all
                "id": "sz-3",
                "title": "Plain talk",
                "categories": [],
            },
        ],
        "metadata": {},
    }
    conn.execute(
        "INSERT INTO sessionize_source_cache (id, speakers_url, sessions_url, payload)"
        " VALUES (1, 'a', 'b', ?)",
        (json.dumps(payload),),
    )
    conn.executemany(
        "INSERT INTO event_schedules (id, event_slug, session_id, title) VALUES (?, 'cd-2026', ?, ?)",
        [
            ("m-bilingual", "sz-1", "Bilingual talk"),
            ("m-level", "sz-2", "Leveled talk"),
            ("m-plain", "sz-3", "Plain talk"),
            ("m-manual", "sz-1", "Manual languages"),  # same session, second row
            ("m-csv", None, "Manually seeded CSV row"),
            ("m-ghost", "sz-404", "No cache match"),
        ],
    )
    conn.commit()

    apply(conn, "0030_schedule_languages_levels.sql")

    rows = conn.execute(
        "SELECT id, languages_json, levels_json FROM event_schedules ORDER BY id"
    ).fetchall()
    by_id = {r[0]: (json.loads(r[1]), json.loads(r[2])) for r in rows}

    assert by_id["m-bilingual"] == (["English", "Bahasa Indonesia"], []), by_id["m-bilingual"]
    assert by_id["m-level"] == (["English"], ["Advanced"]), by_id["m-level"]
    assert by_id["m-plain"] == ([], []), by_id["m-plain"]
    assert by_id["m-manual"] == (["English", "Bahasa Indonesia"], []), by_id["m-manual"]
    assert by_id["m-csv"] == ([], []), "rows without session_id stay []"
    assert by_id["m-ghost"] == ([], []), "unmatched session_id stays []"

    # Job Title category on the session must not leak into either array.
    assert "Developer Advocate" not in by_id["m-bilingual"][0] + by_id["m-bilingual"][1]

    # The UPDATE-only part is idempotent (guarded on '[]'): a manual edit
    # after the migration survives re-running the backfill statements.
    conn.execute(
        "UPDATE event_schedules SET languages_json = '[\"Manual\"]',"
        " levels_json = '[\"Beginner\"]' WHERE id = 'm-bilingual'"
    )
    update_sql = (MIGRATIONS / "0030_schedule_languages_levels.sql").read_text()
    start = update_sql.index("UPDATE event_schedules")
    conn.executescript(update_sql[start:])
    after = conn.execute(
        "SELECT languages_json, levels_json FROM event_schedules WHERE id = 'm-bilingual'"
    ).fetchone()
    assert after == ('["Manual"]', '["Beginner"]'), after

    print("check_schedule_languages: OK")


if __name__ == "__main__":
    main()
