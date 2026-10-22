#!/usr/bin/env python3
"""Source-level regression check for public-endpoint cache headers.

The real header behavior cannot be asserted in `cargo test`: `worker::Headers`
is web-sys backed and panics on non-wasm targets. This check instead pins the
source contract the wasm build compiles:

- `apply_public_cache_headers` stamps the agreed window
  `public, max-age=120, stale-while-revalidate=300` and restores
  `Content-Type: application/json; charset=UTF-8` (with_headers replaces it).
- Only the two public helpers (`json_public_cached`, `with_public_cache`) use it.
- Shared helpers used by admin/auth responses (`json_success`, `json_success_cors`,
  `with_cors`) stay cache-free.
- `handle_public_speakers` / `handle_public_schedule` are the only handlers
  routed through the cached helpers.

Run: python3 scripts/check_public_cache_headers.py
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RESPONSE_RS = ROOT / "src" / "http" / "response.rs"
SPEAKERS_RS = ROOT / "src" / "http" / "speakers.rs"
SCHEDULE_RS = ROOT / "src" / "http" / "schedule.rs"

CACHE_WINDOW = "public, max-age=120, stale-while-revalidate=300"
CONTENT_TYPE = "application/json; charset=UTF-8"


def fn_body(src: str, signature: str) -> str:
    """Return the brace-balanced body source of the function matching `signature`."""
    m = re.search(re.escape(signature), src)
    assert m, f"signature not found: {signature}"
    start = src.index("{", m.end() - 1)
    depth = 0
    for i in range(start, len(src)):
        if src[i] == "{":
            depth += 1
        elif src[i] == "}":
            depth -= 1
            if depth == 0:
                return src[start : i + 1]
    raise AssertionError(f"unbalanced braces in {signature}")


def handlers_using(src: str, helper: str) -> set[str]:
    """Names of `pub async fn handle_*` whose body calls `helper(`."""
    used = set()
    for m in re.finditer(r"pub async fn (handle_\w+)", src):
        if re.search(rf"\b{re.escape(helper)}\s*\(", fn_body(src, m.group(0))):
            used.add(m.group(1))
    return used


def main() -> None:
    response = RESPONSE_RS.read_text()

    apply_body = fn_body(response, "fn apply_public_cache_headers")
    assert CACHE_WINDOW in apply_body, "cache window missing in apply_public_cache_headers"
    assert CONTENT_TYPE in apply_body, "content-type restore missing in apply_public_cache_headers"

    # Cache window appears exactly once in the file — only via the public helper.
    assert response.count(CACHE_WINDOW) == 1, "cache window duplicated or hoisted elsewhere"

    # Only the two public helpers may call apply_public_cache_headers.
    callers = {m.group(1) for m in re.finditer(r"pub fn (\w+)", response)}
    cached_callers = {
        name
        for name in callers
        if "apply_public_cache_headers" in fn_body(response, f"pub fn {name}")
    }
    assert cached_callers == {"json_public_cached", "with_public_cache"}, (
        f"unexpected callers of apply_public_cache_headers: {cached_callers}"
    )

    # Shared admin/auth helpers must remain cache-free.
    for shared in ("pub fn json_success", "pub fn json_success_cors", "pub fn with_cors"):
        assert "Cache-Control" not in fn_body(response, shared), f"{shared} must not cache"

    speakers = SPEAKERS_RS.read_text()
    assert handlers_using(speakers, "with_public_cache") == {"handle_public_speakers"}, (
        "with_public_cache must be used only by the public speakers handler"
    )

    schedule = SCHEDULE_RS.read_text()
    assert handlers_using(schedule, "json_public_cached") == {"handle_public_schedule"}, (
        "json_public_cached must be used only by the public schedule handler"
    )

    print("ok: public speakers/schedule carry bounded cache; admin/auth helpers untouched")


if __name__ == "__main__":
    main()
