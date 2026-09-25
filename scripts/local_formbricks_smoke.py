#!/usr/bin/env python3
"""Local smoke test for the Formbricks pagination walk against a real wrangler dev Worker.

Fake upstream + throwaway local D1 (--persist-to tempdir). No real credentials,
no network beyond loopback, no writes to .wrangler/state. Requires node >= 22
on PATH (nvm use 22). Stdlib only.

Usage: python3 scripts/local_formbricks_smoke.py
Exit 0 = all checks passed.
"""
import json
import os
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request
import base64
import hashlib
import hmac
import secrets
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADMIN_EMAIL = "smoke-admin@example.com"
AUTH = {"X-Debug-User-Email": ADMIN_EMAIL}

# --- fixtures -----------------------------------------------------------------

def make_response(rid, finished=True):
    return {
        "id": rid, "surveyId": "",  # stamped per-survey at serve time
        "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
        "finished": finished,
        "data": {"q-email": "smoke@example.com"}, "contact": None,
    }

def stats_ts(i):
    return f"2026-05-01T{i // 60:02d}:{i % 60:02d}:00Z"

def stats_response(i):
    ts = stats_ts(i)
    return {**make_response(f"t{i:03d}", finished=(i % 2 == 0)),
            "createdAt": ts, "updatedAt": ts}

SURVEYS = {
    # meta omitted entirely -> old code failed here
    "bugcase": {"responses": [make_response("b1"), make_response("b2", finished=False)], "meta": None},
    # meta present but total null
    "nulltotal": {"responses": [make_response("n1"), make_response("n2", finished=False)], "meta": {"total": None}},
    # normal: total present
    "okcase": {"responses": [make_response("r1"), make_response("r2", finished=False)], "meta": {"total": 2}},
    # 120 responses, server caps pages at 50 even when limit=100 is requested
    "capcase": {"responses": [make_response(f"c{i:03d}") for i in range(120)], "meta": {"total": 120}, "page_cap": 50},
    # 130 responses, mixed finished (even finished), meta omitted; latest
    # timestamp sits on index 129, beyond both the walk page and the page-1 window
    "statscase": {"responses": [stats_response(i) for i in range(130)], "meta": None},
    # empty survey owned by the inactive volunteer (Talent Pool) form; the
    # webhook test injects its response here before firing the webhook
    "talentcase": {"responses": [], "meta": None},
}

def survey_body(sid):
    return {"data": {"id": sid, "name": "Smoke " + sid, "status": "in progress",
                     "questions": [{"id": "q-email", "headline": {"default": "Email"}, "type": "email"}]}}

PAGE_HITS = []  # (surveyId, limit, skip) in order — inspected to prove the no-gap walk

class MockHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        u = urlparse(self.path)
        q = parse_qs(u.query)
        if u.path.startswith("/api/v1/management/surveys/"):
            body = survey_body(u.path.rsplit("/", 1)[1])
        elif u.path == "/api/v2/management/responses":
            sid = q.get("surveyId", [""])[0]
            if sid not in SURVEYS:
                self.send_response(404); self.end_headers(); return
            limit = int(q.get("limit", ["100"])[0])
            skip = int(q.get("skip", ["0"])[0])
            PAGE_HITS.append((sid, limit, skip))
            fx = SURVEYS[sid]
            page = [{**r, "surveyId": sid} for r in fx["responses"][skip:skip + min(limit, fx.get("page_cap", 10**9))]]
            body = {"data": page}
            if fx["meta"] is not None:
                body["meta"] = fx["meta"]
        elif u.path.startswith("/api/v2/management/responses/"):
            rid = u.path.rsplit("/", 1)[1]
            owner = next((sid for sid, fx in SURVEYS.items() for r in fx["responses"] if r["id"] == rid), None)
            if owner is None:
                self.send_response(404); self.end_headers(); return
            body = {"data": {**next(r for r in SURVEYS[owner]["responses"] if r["id"] == rid), "surveyId": owner}}
        else:
            self.send_response(404); self.end_headers(); return
        payload = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *a):
        pass

# --- helpers ------------------------------------------------------------------

RESULTS = []

def check(name, ok, detail=""):
    RESULTS.append((name, ok))
    print(f"{'PASS' if ok else 'FAIL'}  {name}" + (f"  [{detail}]" if detail else ""), flush=True)

def http(method, url, headers=None, body=None, timeout=30):
    data = (body.encode() if isinstance(body, str)
            else json.dumps(body).encode()) if body is not None else None
    req = urllib.request.Request(url, data=data, method=method,
                                 headers={"Content-Type": "application/json", **(headers or {})})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            raw = r.read()
            try:
                return r.status, json.loads(raw or b"null")
            except json.JSONDecodeError:
                return r.status, raw.decode(errors="replace")
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw or b"null")
        except json.JSONDecodeError:
            return e.code, raw.decode(errors="replace")
    except urllib.error.URLError:
        return 0, None  # connection refused / not up yet

def free_port():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p

def run(cmd, timeout, log_path=None):
    with open(log_path, "wb") if log_path else open(os.devnull, "wb") as out:
        return subprocess.run(cmd, cwd=ROOT, stdout=out, stderr=subprocess.STDOUT, timeout=timeout).returncode

# --- talent pool / webhook helpers --------------------------------------------

WEBHOOK_SECRET = base64.b64encode(secrets.token_bytes(32)).decode()  # passed as whsec_<b64>

def sign_webhook(body: str):
    """Return headers for a valid Formbricks webhook signature over `body`."""
    wh_id = "msg_" + secrets.token_hex(8)
    ts = str(int(time.time()))
    mac = hmac.new(base64.b64decode(WEBHOOK_SECRET),
                   f"{wh_id}.{ts}.{body}".encode(), hashlib.sha256).digest()
    return {"webhook-id": wh_id, "webhook-timestamp": ts,
            "webhook-signature": "v1," + base64.b64encode(mac).decode()}

def post_webhook(base_url: str, payload: dict):
    body = json.dumps(payload)
    return http("POST", base_url + "/api/webhook/formbricks", sign_webhook(body), body)

def webhook_payload(rid: str, survey: str, email: str, linkedin: str):
    return {"event": "responseFinished", "webhookId": "wh_" + rid,
            "data": {"id": rid, "surveyId": survey, "finished": True,
                     "createdAt": "2026-06-01T00:00:00Z",
                     "data": {"q-email": email, "q-linkedin": linkedin}}}

def add_survey_response(sid: str, rid: str, email: str, linkedin: str):
    """Inject a response into a fixture survey so fake get_response serves it."""
    SURVEYS[sid]["responses"].append(
        {**make_response(rid), "data": {"q-email": email, "q-linkedin": linkedin}})

def form_by_slug(forms: list, slug: str):
    return next((f for f in forms if f.get("slug") == slug), None)

# --- main ---------------------------------------------------------------------

def main():
    state = tempfile.mkdtemp(prefix="fb_smoke_state_")
    worker_port, mock_port = free_port(), free_port()
    base = f"http://127.0.0.1:{worker_port}"
    api = base + "/api/admin/formbricks"
    dev = None
    mock = ThreadingHTTPServer(("127.0.0.1", mock_port), MockHandler)
    threading.Thread(target=mock.serve_forever, daemon=True).start()
    signal.signal(signal.SIGALRM, lambda *a: (_ for _ in ()).throw(TimeoutError("overall 15min cap")))
    signal.alarm(900)
    try:
        wrangler = ["npx", "--yes", "wrangler@4"]
        rc = run(wrangler + ["d1", "migrations", "apply", "jakarta-backend",
                             "--local", "--persist-to", state], 240,
                 os.path.join(state, "migrate.log"))
        check("d1 migrations apply (temp state)", rc == 0)

        # Seed forms covering every policy combination under test:
        # active volunteer, INACTIVE volunteer (Talent Pool), inactive speaker,
        # a 130-response survey (>50) and a survey the fake upstream 404s.
        seed_sql = os.path.join(state, "seed.sql")
        with open(seed_sql, "w") as f:
            f.write(
                "INSERT INTO application_forms (id, kind, slug, title, formbricks_survey_id, "
                "formbricks_public_url, email_question_id, linkedin_question_id, is_active, display_order) VALUES\n"
                "('f-vact','volunteer','vact','Volunteer Active','okcase','https://forms.example/s/vact','q-email','q-linkedin',1,1),\n"
                "('f-vtal','volunteer','vtalent','Volunteer Talent','talentcase','https://forms.example/s/vtalent','q-email','q-linkedin',0,2),\n"
                "('f-spk','speaker','spk','Speaker Inactive','bugcase','https://forms.example/s/spk','q-email','q-linkedin',0,1),\n"
                "('f-vstats','volunteer','vstats','Volunteer Stats','statscase','https://forms.example/s/vstats','q-email','q-linkedin',1,3),\n"
                "('f-vdead','volunteer','vdead','Volunteer Dead','ghost','https://forms.example/s/vdead','q-email','q-linkedin',1,4);\n")
        rc = run(wrangler + ["d1", "execute", "jakarta-backend", "--local",
                             "--persist-to", state, "--file", seed_sql], 240,
                 os.path.join(state, "seed.log"))
        check("d1 seed forms", rc == 0)

        dev_log = os.path.join(state, "wrangler.log")
        dev = subprocess.Popen(
            wrangler + ["dev", "--local", "--persist-to", state, "--port", str(worker_port),
                        "--var", f"FORMBRICKS_BASE_URL:http://127.0.0.1:{mock_port}",
                        "--var", "FORMBRICKS_API_KEY:fake-smoke-key",
                        "--var", f"PRETIX_API_BASE_URL:http://127.0.0.1:{mock_port}",
                        "--var", "PRETIX_API_TOKEN:fake-smoke-token",
                        "--var", "ENABLE_DEBUG_AUTH:true",
                        "--var", f"ADMIN_EMAILS:{ADMIN_EMAIL}",
                        "--var", f"FORMBRICKS_WEBHOOK_SECRET:whsec_{WEBHOOK_SECRET}"],
            cwd=ROOT, stdout=open(dev_log, "wb"), stderr=subprocess.STDOUT,
            start_new_session=True)
        up = any(http("GET", base + "/health")[0] == 200 for _ in range(120) if not time.sleep(2))
        check("worker /health up", up, f"log: {dev_log}")

        # 1. auth enforced
        check("no auth header -> 401", http("GET", api + "/responses?surveyId=okcase")[0] == 401)

        # 2/3. missing meta + null total on the filtered (walk) path
        for sid in ("bugcase", "nulltotal"):
            st, body = http("GET", f"{api}/responses?surveyId={sid}&finished=true", AUTH)
            check(f"{sid} finished=true -> 200", st == 200, str(body)[:120])
            check(f"{sid} filtered total==1", st == 200 and body.get("total") == 1)

        # 4. capcase: server caps pages at 50 despite limit=100, 120 total
        st, p1 = http("GET", f"{api}/responses?surveyId=capcase&finished=true&limit=100", AUTH)
        skips = [s for sid, _, s in PAGE_HITS if sid == "capcase"]
        st2, p2 = http("GET", f"{api}/responses?surveyId=capcase&finished=true&limit=100&offset=100", AUTH)
        ids = {i["id"] for i in p1.get("items", [])} | {i["id"] for i in p2.get("items", [])}
        expected = {f"c{i:03d}" for i in range(120)}
        check("capcase total==120", p1.get("total") == 120, f"total={p1.get('total')}")
        check("capcase all 120 ids, no gaps/dupes", ids == expected, f"got {len(ids)} unique")
        check("capcase walk advanced by received page size", skips == [0, 50, 100],
              f"skips={skips}")

        # 4b. statscase: stats span all pages with meta omitted; latest beyond page 1
        st, p1 = http("GET", f"{api}/responses?surveyId=statscase&limit=50", AUTH)
        s1 = p1.get("stats") or {}
        check("statscase page1 items=50 stats 130/65/65",
              st == 200 and len(p1.get("items", [])) == 50
              and (s1.get("total"), s1.get("finished"), s1.get("in_progress")) == (130, 65, 65),
              str(s1)[:120])
        check("statscase page1 latest beyond window", s1.get("latest_submission") == stats_ts(129),
              str(s1)[:120])
        st, p2 = http("GET", f"{api}/responses?surveyId=statscase&limit=50&offset=50", AUTH)
        check("statscase page2 stats identical", st == 200 and p2.get("stats") == s1)
        st, pe = http("GET", f"{api}/responses?surveyId=statscase&limit=50&offset=5000", AUTH)
        check("statscase empty page stats identical",
              st == 200 and pe.get("items") == [] and pe.get("stats") == s1)
        st, pf = http("GET", f"{api}/responses?surveyId=statscase&finished=true&limit=50", AUTH)
        sf = (pf.get("stats") or {}) if st == 200 else {}
        check("statscase finished=true stats filtered",
              st == 200 and (sf.get("total"), sf.get("finished"), sf.get("in_progress"),
                             sf.get("latest_submission")) == (65, 65, 0, stats_ts(128)),
              str(sf)[:120])

        # 5. tag PUT (verified via fake get_response) + 6. tag-filtered listing
        st, body = http("PUT", f"{api}/responses/r1/tags?surveyId=okcase", AUTH, {"tags": ["Shortlisted"]})
        check("PUT tags r1 -> 200", st == 200 and body.get("tags") == ["Shortlisted"], str(body)[:120])
        st, body = http("GET", f"{api}/responses?surveyId=okcase&tag=Shortlisted", AUTH)
        got = [i["id"] for i in body.get("items", [])]
        check("tag-filtered listing -> only r1", st == 200 and got == ["r1"] and body.get("total") == 1,
              f"ids={got}")

        # 7. detail endpoint (fake get_response path) reflects D1 tags
        st, body = http("GET", f"{api}/responses/r1?surveyId=okcase", AUTH)
        check("detail r1 -> 200 with tag", st == 200 and "Shortlisted" in body.get("tags", []))
        st, body = http("GET", f"{api}/tags?surveyId=okcase", AUTH)
        check("survey tag catalog contains label", st == 200 and "Shortlisted" in (body if isinstance(body, list) else []))

        # 8. regression guard: unfiltered single-page path still fine with missing meta
        st, _ = http("GET", f"{api}/responses?surveyId=bugcase", AUTH)
        check("bugcase unfiltered -> 200", st == 200)

        # 9. multi-tag OR filter + untagged + combo rejection (capcase, 120 live)
        for rid, tag in (("c000", "Alpha"), ("c001", "Alpha"), ("c002", "Beta"), ("c100", "Gamma")):
            st, body = http("PUT", f"{api}/responses/{rid}/tags?surveyId=capcase", AUTH, {"tags": [tag]})
            check(f"PUT {tag} on {rid} -> 200", st == 200)
        st, body = http("GET", f"{api}/responses?surveyId=capcase&tag=Alpha&tag=Beta", AUTH)
        got = sorted(i["id"] for i in body.get("items", []))
        check("multi tag OR -> Alpha+Beta total 3", st == 200 and body.get("total") == 3
              and got == ["c000", "c001", "c002"], f"got={got}")
        st, body = http("GET", f"{api}/responses?surveyId=capcase&tag=Alpha&tag=Beta&finished=true", AUTH)
        check("multi tag + finished combo -> 3", st == 200 and body.get("total") == 3
              and (body.get("stats") or {}).get("total") == 3)
        st, body = http("GET", f"{api}/responses?surveyId=capcase&untagged=true", AUTH)
        check("untagged -> 116 of 120", st == 200 and body.get("total") == 116,
              f"total={body.get('total')}")
        st, body = http("GET", f"{api}/responses?surveyId=capcase&untagged=true&limit=10", AUTH)
        check("untagged stats span full set, not window",
              st == 200 and len(body.get("items", [])) == 10
              and (body.get("stats") or {}).get("total") == 116)
        st, _ = http("GET", f"{api}/responses?surveyId=capcase&tag=Alpha&untagged=true", AUTH)
        check("tag + untagged -> 400", st == 400)
        st, body = http("GET", f"{api}/responses?surveyId=capcase&tag=Gamma", AUTH)
        check("single tag param (legacy) -> 1", st == 200 and body.get("total") == 1)
        st, body = http("GET", f"{api}/responses?surveyId=capcase", AUTH)
        check("no tag params (legacy) -> all 120", st == 200 and body.get("total") == 120)

        # 10. admin forms response_count from ALL live Formbricks responses
        st, forms = http("GET", base + "/api/admin/forms", AUTH)
        by_slug = {f.get("slug"): f for f in forms if isinstance(f, dict)}
        check("admin forms list -> 200", st == 200 and len(by_slug) == 5, str(len(forms)))
        check("vstats count is live 130 (>50), not D1 index 0",
              by_slug.get("vstats", {}).get("response_count") == 130,
              str(by_slug.get("vstats", {}).get("response_count")))
        check("vact live count 2", by_slug.get("vact", {}).get("response_count") == 2)
        check("vdead upstream failure -> null count",
              by_slug.get("vdead", {}).get("response_count") is None)
        # toggle must not fail when the count fetch fails (state already mutated)
        st, body = http("PUT", base + "/api/admin/forms/volunteer/vdead", AUTH, {"is_active": True})
        check("toggle vdead -> 200 with null count",
              st == 200 and body.get("response_count") is None and body.get("is_active") is True,
              str(body)[:200])
        st, body = http("PUT", base + "/api/admin/forms/volunteer/vtalent", AUTH, {"is_active": False})
        check("toggle vtalent -> 200 with live count 0",
              st == 200 and body.get("response_count") == 0, str(body)[:200])

        # 11. public listings expose inactive volunteer (Talent Pool), not inactive speaker
        st, vforms = http("GET", base + "/api/forms?kind=volunteer")
        vt = form_by_slug(vforms if isinstance(vforms, list) else [], "vtalent")
        check("public volunteer list includes inactive vtalent",
              st == 200 and vt is not None and vt.get("is_active") is False, str(vt)[:200])
        st, sforms = http("GET", base + "/api/forms?kind=speaker")
        slugs = [f.get("slug") for f in sforms if isinstance(f, dict)]
        check("public speaker list excludes inactive spk",
              st == 200 and "spk" not in slugs, str(slugs))
        st, body = http("GET", base + "/api/forms/volunteer/vtalent")
        check("inactive volunteer form reachable, status open (Talent Pool)",
              st == 200 and body.get("form", {}).get("is_active") is False
              and body.get("status") == "open", str(body)[:200])
        st, body = http("GET", base + "/api/forms/speaker/spk")
        check("inactive speaker form status closed",
              st == 200 and body.get("status") == "closed", str(body)[:200])
        st, body = http("GET", base + "/api/forms/volunteer/vact")
        check("active volunteer form status open",
              st == 200 and body.get("status") == "open", str(body)[:200])

        # 12. link endpoints: Talent Pool submission allowed, inactive speaker blocked
        st, body = http("POST", base + "/api/applications/volunteer/vtalent/link",
                        {"X-Debug-User-Email": "talent@example.com"})
        check("inactive volunteer link -> 200 with url",
              st == 200 and "forms.example" in str(body.get("url", ""))
              and body.get("editable") is True, str(body)[:200])
        st, body = http("POST", base + "/api/applications/speaker/spk/link",
                        {"X-Debug-User-Email": "spk@example.com"})
        check("inactive speaker link -> 403", st == 403)

        # 13. webhook: inactive volunteer submission indexed + tagged Talent Pool
        add_survey_response("talentcase", "wbt1", "Talent@Example.com",
                            "https://www.linkedin.com/in/talent-one")
        st, body = http("PUT", f"{api}/responses/wbt1/tags?surveyId=talentcase",
                        AUTH, {"tags": ["Manual Pick"]})
        check("manual tag on wbt1 before webhook", st == 200)
        st, txt = post_webhook(base, webhook_payload("wbt1", "talentcase",
                                                     "Talent@Example.com",
                                                     "https://www.linkedin.com/in/talent-one"))
        check("webhook talent pool -> 200", st == 200, str(txt)[:200])
        st, body = http("GET", base + "/api/applications/volunteer/vtalent",
                        {"X-Debug-User-Email": "talent@example.com"})
        check("talent pool submission indexed + discoverable",
              st == 200 and body.get("exists") is True and body.get("response_id") == "wbt1"
              and body.get("editable") is True, str(body)[:300])
        st, body = http("GET", f"{api}/responses/wbt1?surveyId=talentcase", AUTH)
        check("wbt1 tags = Manual Pick + Talent Pool (additive)",
              st == 200 and sorted(body.get("tags", [])) == ["Manual Pick", "Talent Pool"],
              str(body.get("tags", [])))
        # idempotent: same webhook again must not duplicate the tag
        st, _ = post_webhook(base, webhook_payload("wbt1", "talentcase",
                                                   "Talent@Example.com",
                                                   "https://www.linkedin.com/in/talent-one"))
        st, body = http("GET", f"{api}/responses/wbt1?surveyId=talentcase", AUTH)
        check("webhook retry keeps single Talent Pool tag",
              st == 200 and body.get("tags", []).count("Talent Pool") == 1
              and sorted(body.get("tags", [])) == ["Manual Pick", "Talent Pool"])
        # edit link for the talent pool response works while form inactive
        st, body = http("POST", base + "/api/applications/volunteer/vtalent/link?mode=edit",
                        {"X-Debug-User-Email": "talent@example.com"})
        check("talent pool edit link -> 200",
              st == 200 and body.get("editable") is True, str(body)[:200])

        # 14. webhook: active volunteer submission indexed but NOT talent-tagged
        add_survey_response("okcase", "wbt2", "active@example.com",
                            "https://www.linkedin.com/in/active-one")
        st, _ = post_webhook(base, webhook_payload("wbt2", "okcase",
                                                   "active@example.com",
                                                   "https://www.linkedin.com/in/active-one"))
        st, body = http("GET", f"{api}/responses/wbt2?surveyId=okcase", AUTH)
        check("active volunteer response has NO talent pool tag",
              st == 200 and body.get("tags", []) == [], str(body.get("tags", [])))

        # 15. webhook: inactive speaker submission ignored (not indexed)
        add_survey_response("bugcase", "wbs1", "spk@example.com",
                            "https://www.linkedin.com/in/spk-one")
        st, txt = post_webhook(base, webhook_payload("wbs1", "bugcase",
                                                     "spk@example.com",
                                                     "https://www.linkedin.com/in/spk-one"))
        check("inactive speaker webhook -> 200 ignored", st == 200, str(txt)[:200])
        st, body = http("GET", base + "/api/applications/speaker/spk",
                        {"X-Debug-User-Email": "spk@example.com"})
        check("inactive speaker submission NOT indexed",
              st == 200 and body.get("exists") is False, str(body)[:200])

        # 16. talent pool tag failure -> 500 retryable; index survives; retry tags
        add_survey_response("talentcase", "wbt3", "talent3@example.com",
                            "https://www.linkedin.com/in/talent-three")
        drop_sql = os.path.join(state, "drop_tags.sql")
        with open(drop_sql, "w") as f:
            f.write("DROP TABLE response_tags;\n")
        rc = run(wrangler + ["d1", "execute", "jakarta-backend", "--local",
                             "--persist-to", state, "--file", drop_sql], 240,
                 os.path.join(state, "drop.log"))
        check("drop response_tags for failure injection", rc == 0)
        st, txt = post_webhook(base, webhook_payload("wbt3", "talentcase",
                                                      "talent3@example.com",
                                                      "https://www.linkedin.com/in/talent-three"))
        check("talent pool tag failure -> 500 retryable", st == 500, f"st={st} {str(txt)[:120]}")
        st, body = http("GET", base + "/api/applications/volunteer/vtalent",
                        {"X-Debug-User-Email": "talent3@example.com"})
        check("wbt3 indexed despite tag failure (retry-safe)",
              st == 200 and body.get("exists") is True and body.get("response_id") == "wbt3",
              str(body)[:200])
        rc = run(wrangler + ["d1", "execute", "jakarta-backend", "--local",
                             "--persist-to", state, "--file",
                             os.path.join(ROOT, "migrations", "0015_response_tags.sql")], 240,
                 os.path.join(state, "recreate.log"))
        check("recreate response_tags", rc == 0)
        st, _ = post_webhook(base, webhook_payload("wbt3", "talentcase",
                                                   "talent3@example.com",
                                                   "https://www.linkedin.com/in/talent-three"))
        check("webhook retry after fix -> 200", st == 200)
        st, body = http("GET", f"{api}/responses/wbt3?surveyId=talentcase", AUTH)
        check("retry tagged wbt3 Talent Pool",
              st == 200 and body.get("tags") == ["Talent Pool"], str(body.get("tags", [])))
    finally:
        signal.alarm(0)
        if dev is not None:
            try:
                os.killpg(os.getpgid(dev.pid), signal.SIGTERM)
                dev.wait(timeout=10)
            except Exception:
                try:
                    os.killpg(os.getpgid(dev.pid), signal.SIGKILL)
                except Exception:
                    pass
        mock.shutdown()
        print(f"\nstate dir (kept for logs): {state}")
    passed = sum(1 for _, ok in RESULTS if ok)
    print(f"\n{passed}/{len(RESULTS)} checks passed")
    if passed != len(RESULTS):
        sys.exit(1)

if __name__ == "__main__":
    main()
