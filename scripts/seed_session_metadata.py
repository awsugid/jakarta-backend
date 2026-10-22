#!/usr/bin/env python3
"""Seed event_schedules.languages_json / levels_json for community-day-2026
from the live Sessionize Sessions API (accepted sessions only).

Dry-run by default: fetches Sessionize via urllib (stdlib), takes a READ-ONLY
remote D1 snapshot (wrangler d1 execute --remote --json, SELECTs only), plans
idempotent UPDATEs, writes SQL to a tempfile OUTSIDE the repo. --apply first
writes a backup tempfile of the affected rows' metadata + publication
fingerprint, then executes the SQL file remotely, then re-reads and verifies.

Rules (agreed with user):
  - match DB rows by event_slug='community-day-2026' AND session_id ONLY —
    no title matching, no guesses; upstream session ids compared as strings
  - upstream filter: status == "accepted" (case-insensitive)
  - category matching mirrors src/sessionize/types.rs:
      languages: category name in {"language", "languages"}
      levels:    category name in {"level", "session level",
                                  "experience level", "audience level"}
    (name trim + lower; item values trimmed, empty skipped, deduped, order kept)
  - fill languages_json/levels_json ONLY when the column is '[]' AND the
    upstream list is non-empty; manual admin edits are never overwritten
  - other fields (assignments/timing/is_active/format/publication) untouched;
    revision = revision + 1, updated_at = datetime('now')
  - workspace revision guard first (CASE -> NULL violates NOT NULL on drift),
    per-row UPDATE ... AND revision = <snapshot> CAS after that
  - migration 0030 must already be applied remotely — verified first; never
    run migrations from here; bounded wait + report if it is not there yet
  - rerun after a successful apply plans ZERO statements (natural verify)

Stdlib only. Python 3.9+. No credentials are logged (secret-ish strings are
redacted before any error output).
"""
import argparse
import hashlib
import json
import os
import re
import shlex
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

EVENT_SLUG = "community-day-2026"
SESSIONS_URL = "https://sessionize.com/api/v2/egep9rxt/view/Sessions"
MIGRATION_NAME = "0030_schedule_languages_levels"

LANG_CATS = {"language", "languages"}
LEVEL_CATS = {"level", "session level", "experience level", "audience level"}

SECRETISH_RE = re.compile(r"[A-Za-z0-9_\-./+=]{32,}")

WRANGLER_PREAMBLE = (
    ". /home/avei/.nvm/nvm.sh >/dev/null 2>&1 && "
    "nvm use 22 >/dev/null 2>&1 && "
    "npx --yes wrangler@4.151.3 d1 execute jakarta-backend --remote --json "
)
WRANGLER_MIG_LIST = (
    ". /home/avei/.nvm/nvm.sh >/dev/null 2>&1 && "
    "nvm use 22 >/dev/null 2>&1 && "
    "npx --yes wrangler@4.151.3 d1 migrations list jakarta-backend --remote"
)

SNAPSHOT_SQL = ";".join(
    [
        "SELECT id, session_id, title, languages_json, levels_json, revision"
        f" FROM event_schedules WHERE event_slug = '{EVENT_SLUG}' AND session_id IS NOT NULL",
        f"SELECT revision FROM event_schedule_workspace WHERE event_slug = '{EVENT_SLUG}'",
        f"SELECT snapshot_json, published_at FROM event_schedule_publications WHERE event_slug = '{EVENT_SLUG}'",
        "SELECT COUNT(*) AS n FROM event_schedules"
        f" WHERE event_slug = '{EVENT_SLUG}' AND session_id IS NOT NULL"
        " AND (languages_json <> '[]' OR levels_json <> '[]')",
    ]
)


class SeedError(Exception):
    pass


def die(msg):
    raise SeedError(msg)


def q(v):
    """SQL literal (SQLite quoting). Ints stay bare, None -> NULL."""
    if v is None:
        return "NULL"
    if isinstance(v, bool):
        return "1" if v else "0"
    if isinstance(v, int):
        return str(v)
    return "'" + str(v).replace("'", "''") + "'"


def qjson(v):
    return q(json.dumps(v, ensure_ascii=False))


def redact(s):
    return SECRETISH_RE.sub("[redacted]", s)


# ------------------------------------------------------------ wrangler / remote


def run_bash(cmd, timeout=300):
    proc = subprocess.run(
        ["bash", "-c", cmd], cwd=str(ROOT), capture_output=True, text=True, timeout=timeout
    )
    return proc


def run_wrangler(args_sql, use_file=None):
    cmd_sql = WRANGLER_PREAMBLE + (
        ("--file " + shlex.quote(str(use_file))) if use_file else ("--command " + shlex.quote(args_sql))
    )
    proc = run_bash(cmd_sql)
    if proc.returncode != 0:
        tail = redact((proc.stderr + proc.stdout)[-2000:])
        die(
            "wrangler failed (rc=%s):\n%s\n"
            "hint: check `npx wrangler@4.151.3 whoami` (OAuth login) and"
            " CLOUDFLARE_ACCOUNT_ID / CLOUDFLARE_API_TOKEN" % (proc.returncode, tail)
        )
    return proc.stdout


def parse_wrangler_json(stdout):
    """wrangler may prefix logs to stdout; find the JSON array robustly."""
    for idx, ch in enumerate(stdout):
        if ch != "[":
            continue
        try:
            doc, _ = json.JSONDecoder().raw_decode(stdout, idx)
            if isinstance(doc, list):
                return doc
        except json.JSONDecodeError:
            continue
    die("could not parse wrangler --json output: " + redact(stdout[-500:]))


def publication_fingerprint(rows):
    """sha256 of the published snapshot_json; None when unpublished."""
    if not rows:
        return None
    snap_json = rows[0].get("snapshot_json") or ""
    return {
        "sha256": hashlib.sha256(snap_json.encode("utf-8")).hexdigest(),
        "bytes": len(snap_json.encode("utf-8")),
        "published_at": rows[0].get("published_at"),
    }


def fetch_snapshot():
    doc = parse_wrangler_json(run_wrangler(SNAPSHOT_SQL))
    if len(doc) < 4:
        die(f"snapshot: expected 4 statement results, got {len(doc)}")
    ws_rows = doc[1].get("results") or []
    if not ws_rows:
        die(f"no event_schedule_workspace row for {EVENT_SLUG!r} — open the workspace once, then retry")
    filled = (doc[3].get("results") or [{}])[0].get("n", 0)
    return {
        "event_slug": EVENT_SLUG,
        "schedules": doc[0].get("results") or [],
        "workspace_revision": int(ws_rows[0]["revision"]),
        "publication": publication_fingerprint(doc[2].get("results") or []),
        "rows_with_metadata": int(filled),
    }


def migration_applied():
    """True when 0030 is NOT listed as pending by `d1 migrations list --remote`."""
    proc = run_bash(WRANGLER_MIG_LIST, timeout=180)
    if proc.returncode != 0:
        tail = redact((proc.stderr + proc.stdout)[-1500:])
        die(
            "wrangler d1 migrations list failed (rc=%s):\n%s\n"
            "hint: check `npx wrangler@4.151.3 whoami` (OAuth login) and"
            " CLOUDFLARE_ACCOUNT_ID / CLOUDFLARE_API_TOKEN" % (proc.returncode, tail)
        )
    return MIGRATION_NAME not in proc.stdout


def wait_for_migration():
    """Bounded wait: one `gh deployment status --watch` attempt (informational,
    deploys run on push to main), then poll the migration list. Never applies."""
    print(f"migration {MIGRATION_NAME} not applied remotely yet — bounded wait")
    proc = run_bash("timeout 240 gh deployment status --watch --exit-status", timeout=300)
    print("gh deployment status: rc=%s %s" % (proc.returncode, redact((proc.stderr or proc.stdout).splitlines()[-1:])))
    for attempt in range(10):
        time.sleep(30)
        if migration_applied():
            print(f"migration {MIGRATION_NAME} now applied (attempt {attempt + 1})")
            return
        print(f"  still pending (attempt {attempt + 1}/10)")
    die(
        f"migration {MIGRATION_NAME} still not applied after bounded wait — "
        "NOT running migrations from here; report to parent (push to main runs "
        "'d1 migrations apply jakarta-backend --remote' via deploy.yml)"
    )


# --------------------------------------------------------------- sessionize IO


def flatten_sessions(doc):
    """Normalize the Sessionize Sessions payload: either a flat session list,
    {'sessions': [...]}, or the grouped view (each element has 'sessions')."""
    if isinstance(doc, dict):
        doc = doc.get("sessions")
    if not isinstance(doc, list):
        die("unexpected Sessionize payload: expected top-level list or {'sessions': [...]}")
    if doc and all(isinstance(g, dict) and "sessions" in g for g in doc):
        doc = [s for g in doc for s in g.get("sessions") or []]
    return doc


def fetch_sessions(url):
    req = urllib.request.Request(url, headers={"User-Agent": "seed_session_metadata.py", "Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as resp:
        doc = json.loads(resp.read().decode("utf-8"))
    return flatten_sessions(doc)


def accepted_sessions(raw):
    out = []
    for s in raw:
        if str(s.get("status") or "").strip().lower() == "accepted":
            out.append(s)
    return out


def extract_meta(categories):
    """(languages, levels) — mirrors sessionize::types::category_item_values."""
    langs, levels = [], []
    seen_l, seen_v = set(), set()
    for cat in categories or []:
        name = str(cat.get("name") or "").strip().lower()
        target = (
            (langs, seen_l) if name in LANG_CATS else (levels, seen_v) if name in LEVEL_CATS else None
        )
        if target is None:
            continue
        out, seen = target
        for item in cat.get("categoryItems") or []:
            value = str(item.get("name") or "").strip()
            if value and value not in seen:
                seen.add(value)
                out.append(value)
    return langs, levels


def parse_list(raw):
    try:
        v = json.loads(raw) if raw else []
    except (TypeError, ValueError):
        die(f"unparseable JSON column value: {raw!r}")
    return v if isinstance(v, list) else die(f"non-list JSON column value: {raw!r}")


# ----------------------------------------------------------------- plan / apply


def plan(sessions, snap):
    by_sid = {}
    for s in sessions:
        sid = str(s.get("id"))
        if sid in by_sid:
            die(f"duplicate upstream session id {sid!r}")
        by_sid[sid] = s

    stmts, updates, backup_rows, plan_lines = [], [], [], []
    unmatched_db, upstream_no_meta, preserved = [], 0, 0
    expected = {}

    for row in snap["schedules"]:
        sid = row.get("session_id")
        src = by_sid.get(str(sid)) if sid is not None else None
        if src is None:
            unmatched_db.append(row)
            continue
        langs, levels = extract_meta(src.get("categories"))
        cur_l = parse_list(row.get("languages_json"))
        cur_v = parse_list(row.get("levels_json"))
        sets = {}
        if not cur_l and langs:
            sets["languages_json"] = langs
        if not cur_v and levels:
            sets["levels_json"] = levels
        if cur_l or cur_v:
            preserved += 1
        if not sets:
            if not (cur_l or cur_v):
                upstream_no_meta += 1
            continue

        backup_rows.append(
            {
                "id": row["id"],
                "session_id": sid,
                "title": row.get("title"),
                "languages_json": row.get("languages_json"),
                "levels_json": row.get("levels_json"),
                "revision": row.get("revision"),
            }
        )
        assignments = ", ".join(f"{k} = {qjson(v)}" for k, v in sets.items())
        updates.append(
            f"UPDATE event_schedules SET {assignments}, revision = revision + 1,"
            " updated_at = datetime('now')"
            f" WHERE event_slug = {q(EVENT_SLUG)} AND id = {q(row['id'])}"
            f" AND revision = {int(row['revision'])}"
        )
        expected[row["id"]] = {
            "languages_json": json.dumps(sets.get("languages_json", cur_l), ensure_ascii=False),
            "levels_json": json.dumps(sets.get("levels_json", cur_v), ensure_ascii=False),
            "title": row.get("title"),
        }
        plan_lines.append(
            f"  {row.get('title')!r} [{row['id']}] -> languages={sets.get('languages_json', cur_l)}"
            f" levels={sets.get('levels_json', cur_v)}"
        )

    stmts = updates
    if stmts:
        guard = (
            "UPDATE event_schedule_workspace SET revision = CASE WHEN revision = "
            f"{snap['workspace_revision']} THEN revision ELSE NULL END"
            f" WHERE event_slug = {q(EVENT_SLUG)}"
        )
        stmts = [guard] + updates

    matched_sids = {str(r.get("session_id")) for r in snap["schedules"]}
    upstream_unmatched = [s.get("title") for s in sessions if str(s.get("id")) not in matched_sids]

    summary = {
        "db_session_rows": len(snap["schedules"]),
        "updates": len(updates),
        "preserved_nonempty": preserved,
        "matched_no_meta_upstream": upstream_no_meta,
        "unmatched_db_rows": len(unmatched_db),
        "unmatched_db_titles": [r.get("title") for r in unmatched_db],
        "upstream_accepted": len(sessions),
        "upstream_without_db_row": len(upstream_unmatched),
        "upstream_without_db_row_titles": upstream_unmatched,
        "workspace_revision": snap["workspace_revision"],
        "rows_with_metadata_before": snap["rows_with_metadata"],
    }
    return stmts, summary, backup_rows, expected, plan_lines


def write_sql(stmts, path=None):
    for s in stmts:
        if s.count(";"):
            die("internal: ';' inside a generated statement would corrupt --file splitting")
    if path is None:
        fd, path = tempfile.mkstemp(prefix="seed_session_metadata_", suffix=".sql")
        with os.fdopen(fd, "w") as f:
            f.write(";\n".join(stmts) + (";\n" if stmts else ""))
    else:
        Path(path).write_text(";\n".join(stmts) + (";\n" if stmts else ""))
    return Path(path)


def write_backup(snap, backup_rows):
    """Tempfile OUTSIDE the repo with pre-apply state of affected rows."""
    fd, path = tempfile.mkstemp(prefix="seed_session_metadata_backup_", suffix=".json")
    with os.fdopen(fd, "w") as f:
        json.dump(
            {
                "event_slug": EVENT_SLUG,
                "workspace_revision": snap["workspace_revision"],
                "publication": snap["publication"],
                "affected_rows": backup_rows,
            },
            f,
            ensure_ascii=False,
            indent=1,
        )
    return Path(path)


def verify(snap_after, expected, snap_before):
    if snap_after["publication"] != snap_before["publication"]:
        die(f"publication fingerprint changed: {snap_before['publication']} -> {snap_after['publication']}")
    rows = {r["id"]: r for r in snap_after["schedules"]}
    for rid, want in expected.items():
        row = rows.get(rid)
        if row is None:
            die(f"verify: row {rid!r} disappeared")
        got = (row.get("languages_json"), row.get("levels_json"))
        if got != (want["languages_json"], want["levels_json"]):
            die(f"verify mismatch on {rid} ({want['title']!r}): got {got}")
    filled = sum(
        1
        for r in snap_after["schedules"]
        if parse_list(r.get("languages_json")) or parse_list(r.get("levels_json"))
    )
    if len(snap_after["schedules"]) != len(snap_before["schedules"]):
        die(
            f"row count changed: {len(snap_before['schedules'])} -> {len(snap_after['schedules'])}"
            " (insert/delete must never happen here)"
        )
    return filled


# ------------------------------------------------------------------- self-test


FIXTURE_DDL = """
CREATE TABLE event_schedules (
  id TEXT PRIMARY KEY, event_slug TEXT NOT NULL, session_id TEXT,
  title TEXT, languages_json TEXT NOT NULL DEFAULT '[]',
  levels_json TEXT NOT NULL DEFAULT '[]',
  revision INTEGER NOT NULL DEFAULT 1, updated_at TEXT
);
CREATE TABLE event_schedule_workspace (
  event_slug TEXT PRIMARY KEY, revision INTEGER NOT NULL
);
CREATE TABLE event_schedule_publications (
  event_slug TEXT PRIMARY KEY, snapshot_json TEXT, published_at TEXT
);
"""


def _apply_stmts(conn, stmts):
    conn.execute("BEGIN")
    for s in stmts:
        conn.execute(s)
    conn.commit()


def self_test():
    conn = sqlite3.connect(":memory:")
    conn.executescript(FIXTURE_DDL)
    conn.execute(
        "INSERT INTO event_schedule_workspace VALUES (?, 7)", (EVENT_SLUG,)
    )
    conn.execute(
        "INSERT INTO event_schedule_publications VALUES (?, ?, '2026-01-01')",
        (EVENT_SLUG, json.dumps({"agenda": "frozen"})),
    )
    conn.executemany(
        "INSERT INTO event_schedules (id, event_slug, session_id, title,"
        " languages_json, levels_json, revision) VALUES (?, ?, ?, ?, ?, ?, ?)",
        [
            ("m-both", EVENT_SLUG, "101", "Fill both", "[]", "[]", 3),
            ("m-manual", EVENT_SLUG, "102", "Manual kept", '["Java"]', "[]", 4),
            ("m-ghost", EVENT_SLUG, "404", "No upstream match", "[]", "[]", 5),
            ("m-nolevel", EVENT_SLUG, "103", "No level upstream", "[]", "[]", 6),
            ("m-nometa", EVENT_SLUG, None, "CSV row", "[]", "[]", 7),
        ],
    )
    conn.commit()

    def sched_rows():
        return [
            dict(
                zip(
                    ("id", "session_id", "title", "languages_json", "levels_json", "revision"),
                    r,
                )
            )
            for r in conn.execute(
                "SELECT id, session_id, title, languages_json, levels_json, revision"
                f" FROM event_schedules WHERE event_slug = '{EVENT_SLUG}' AND session_id IS NOT NULL"
            ).fetchall()
        ]

    def fake_snap():
        return {
            "event_slug": EVENT_SLUG,
            "schedules": sched_rows(),
            "workspace_revision": conn.execute(
                f"SELECT revision FROM event_schedule_workspace WHERE event_slug = '{EVENT_SLUG}'"
            ).fetchone()[0],
            "publication": publication_fingerprint(
                [
                    {"snapshot_json": json.dumps({"agenda": "frozen"}), "published_at": "2026-01-01"}
                ]
            ),
            "rows_with_metadata": 0,
        }

    raw = [
        {  # accepted, multilingual, case-variant category names
            "id": 101,
            "title": "Fill both",
            "status": "Accepted",
            "categories": [
                {"name": "LANGUAGES", "categoryItems": [{"name": " English "}, {"name": "Bahasa Indonesia"}, {"name": "English"}]},
                {"name": " Session Level ", "categoryItems": [{"name": "Intermediate"}]},
                {"name": "Job Title", "categoryItems": [{"name": "Developer"}]},
            ],
        },
        {  # accepted; upstream has language but manual DB value must survive
            "id": "102",
            "title": "Manual kept",
            "status": "accepted",
            "categories": [
                {"name": "Language", "categoryItems": [{"name": "English"}]},
                {"name": "Level", "categoryItems": [{"name": "Beginner"}]},
            ],
        },
        {  # DECLINED — must be filtered out entirely
            "id": 404,
            "title": "No upstream match",
            "status": "Declined",
            "categories": [{"name": "Language", "categoryItems": [{"name": "Klingon"}]}],
        },
        {  # accepted, language only — levels must stay [], nothing fabricated
            "id": 103,
            "title": "No level upstream",
            "status": "Accepted",
            "categories": [{"name": "Language", "categoryItems": [{"name": "Indonesian"}]}],
        },
        {  # accepted upstream with no DB row — informational only
            "id": 104,
            "title": "Orphan upstream",
            "status": "Accepted",
            "categories": [{"name": "Language", "categoryItems": [{"name": "English"}]}],
        },
    ]

    sessions = accepted_sessions(raw)
    assert len(sessions) == 4, [s["id"] for s in sessions]

    # payload shapes: flat list, {'sessions': [...]}, grouped view
    assert flatten_sessions(raw) == raw
    assert flatten_sessions({"sessions": raw}) == raw
    grouped = [{"groupId": 1, "groupName": "All", "isDefault": True, "sessions": raw}]
    assert flatten_sessions(grouped) == raw

    stmts, summary, backup_rows, expected, _ = plan(sessions, fake_snap())
    # m-both (langs+levels) and m-nolevel (langs only); m-manual guarded by
    # non-empty languages... but its levels is [] and upstream has level ->
    # levels DOES fill; languages preserved. Ghost filtered via status. CSV row
    # has no session_id and is not even in the snapshot.
    assert summary["updates"] == 3, summary
    assert summary["unmatched_db_rows"] == 1 and summary["unmatched_db_titles"] == ["No upstream match"]
    assert summary["upstream_without_db_row"] == 1, summary
    assert stmts[0].startswith("UPDATE event_schedule_workspace"), stmts[0]
    assert all(" AND revision = " in s for s in stmts[1:]), stmts
    assert any("revision = 3" in s for s in stmts), "per-row CAS must pin snapshot revision"
    pub_before = conn.execute(
        f"SELECT snapshot_json FROM event_schedule_publications WHERE event_slug = '{EVENT_SLUG}'"
    ).fetchone()[0]

    _apply_stmts(conn, stmts)
    got = {r["id"]: (r["languages_json"], r["levels_json"]) for r in sched_rows()}
    assert got["m-both"] == ('["English", "Bahasa Indonesia"]', '["Intermediate"]'), got["m-both"]
    assert got["m-manual"] == ('["Java"]', '["Beginner"]'), got["m-manual"]
    assert got["m-nolevel"] == ('["Indonesian"]', "[]"), got["m-nolevel"]
    assert got["m-ghost"] == ("[]", "[]"), got["m-ghost"]
    assert conn.execute(
        "SELECT languages_json, levels_json FROM event_schedules WHERE id = 'm-nometa'"
    ).fetchone() == ("[]", "[]"), "row without session_id stays []"
    assert conn.execute(
        f"SELECT snapshot_json FROM event_schedule_publications WHERE event_slug = '{EVENT_SLUG}'"
    ).fetchone()[0] == pub_before, "publication snapshot must be byte-exact"
    assert backup_rows and all("revision" in b for b in backup_rows)

    # idempotency: replanning against post-state yields ZERO statements
    stmts2, summary2, _, _, _ = plan(sessions, fake_snap())
    assert stmts2 == [] and summary2["updates"] == 0, summary2

    # workspace guard: snapshot pinned, THEN revision drifts -> guard violates
    # NOT NULL -> whole batch refuses (fail-closed)
    snap3 = {
        **fake_snap(),
        "schedules": [{**r, "languages_json": "[]", "levels_json": "[]"} for r in sched_rows()],
    }
    conn.execute(
        f"UPDATE event_schedule_workspace SET revision = revision + 1 WHERE event_slug = '{EVENT_SLUG}'"
    )
    conn.commit()
    stmts3, _, _, _, _ = plan(sessions, snap3)
    try:
        _apply_stmts(conn, stmts3)
        raise AssertionError("workspace guard must fail on drift")
    except sqlite3.IntegrityError:
        pass

    # per-row CAS: stale row revision updates 0 rows
    conn.rollback()
    stale = [
        "UPDATE event_schedules SET languages_json = '[\"X\"]', revision = revision + 1,"
        f" updated_at = datetime('now') WHERE event_slug = '{EVENT_SLUG}' AND id = 'm-both' AND revision = 999"
    ]
    before = conn.total_changes
    _apply_stmts(conn, stale)
    assert conn.total_changes == before, "stale CAS must not change anything"

    print("seed_session_metadata self-test: OK")


# ------------------------------------------------------------------------ main


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--apply", action="store_true", help="backup + execute planned SQL remotely via wrangler --file")
    ap.add_argument("--sql-out", help="write planned SQL here (default: tempfile outside repo)")
    ap.add_argument("--self-test", action="store_true", help="local sqlite harness, no network")
    args = ap.parse_args()

    if args.self_test:
        self_test()
        return

    if not migration_applied():
        wait_for_migration()

    sessions = accepted_sessions(fetch_sessions(SESSIONS_URL))
    snap = fetch_snapshot()
    stmts, summary, backup_rows, expected, plan_lines = plan(sessions, snap)
    sql_path = write_sql(stmts, args.sql_out)

    print(f"event: {EVENT_SLUG}  workspace_revision(snapshot): {summary['workspace_revision']}")
    print(
        f"upstream accepted: {summary['upstream_accepted']}"
        f"  db session rows: {summary['db_session_rows']}"
        f"  planned updates: {summary['updates']}"
        f"  preserved-nonempty: {summary['preserved_nonempty']}"
        f"  matched-but-no-upstream-meta: {summary['matched_no_meta_upstream']}"
        f"  unmatched db rows: {summary['unmatched_db_rows']}"
        f"  upstream w/o db row: {summary['upstream_without_db_row']}"
    )
    for line in plan_lines:
        print(line)
    for t in summary["unmatched_db_titles"]:
        print(f"  unmatched db row (kept as-is): {t!r}")
    for t in summary["upstream_without_db_row_titles"]:
        print(f"  upstream accepted without db row (ignored): {t!r}")
    print(f"sql: {sql_path}  ({len(stmts)} statements)")

    if not args.apply:
        print("dry-run only — pass --apply to execute (after backup)")
        return

    backup_path = write_backup(snap, backup_rows)
    print(f"backup (affected rows + publication fingerprint): {backup_path}")

    run_wrangler(None, use_file=sql_path)

    snap_after = fetch_snapshot()
    filled = verify(snap_after, expected, snap)
    print(
        f"verified: {len(expected)}/{len(expected)} rows match planned metadata exactly;"
        f" rows-with-metadata {summary['rows_with_metadata_before']} -> {filled};"
        f" publication fingerprint unchanged"
    )
    for rid, want in expected.items():
        row = next(r for r in snap_after["schedules"] if r["id"] == rid)
        print(f"  {want['title']!r} languages_json={row['languages_json']} levels_json={row['levels_json']} revision={row['revision']}")
    print(
        "note: revision bumps invalidate any outstanding review/publish tokens"
        " (0029 triggers, fail-closed by design); publication NOT touched"
    )


if __name__ == "__main__":
    try:
        main()
    except SeedError as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
