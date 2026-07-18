#!/usr/bin/env python3
"""Discover Formbricks question IDs and generate application_forms seed SQL.

Input mapper JSON shape:
{
  "vol-registration": "https://forms.awscommunity.id/s/kb4bat7ovgkotbhuued4eu52",
  "vol-foh": "https://forms.awscommunity.id/s/xxxx",
  "speaker-main": "https://forms.awscommunity.id/s/yyyy"
}

Environment:
  FORMBRICKS_BASE_URL=https://forms.awscommunity.id
  FORMBRICKS_API_KEY=...

Usage:
  uv run scripts/discover_formbricks_mapping.py \
    --mapper scripts/formbricks_mapper.example.json \
    --output ingest_mapper_formbrick.generated.sql
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class FormTemplate:
    id: str
    kind: str
    slug: str
    title: str
    description: str
    display_order: int


FORM_TEMPLATES: dict[str, FormTemplate] = {
    "vol-registration": FormTemplate(
        "vol-registration",
        "volunteer",
        "registration",
        "Registration",
        "Ensures the on-site re-registration process runs smoothly on event day. After registration is complete, helps prepare and distribute swag kits to all attendees.",
        10,
    ),
    "vol-foh": FormTemplate(
        "vol-foh",
        "volunteer",
        "foh",
        "FOH (Front of House)",
        "Responsible for managing multimedia aspects during the event, including presentation systems, live streaming, broadcasting tools, and smooth execution.",
        20,
    ),
    "vol-logistics": FormTemplate(
        "vol-logistics",
        "volunteer",
        "logistics",
        "Logistics",
        "Manages meals, refreshments, merchandise inventory, stationery supplies, and venue organization.",
        30,
    ),
    "vol-design": FormTemplate(
        "vol-design",
        "volunteer",
        "design",
        "Design",
        "Collaborates on visual assets using Figma and creates materials for AWS UG Jakarta brand touchpoints.",
        40,
    ),
    "vol-documentation": FormTemplate(
        "vol-documentation",
        "volunteer",
        "documentation",
        "Documentation",
        "Captures event moments through photography and videography for community memories and visual content.",
        50,
    ),
    "vol-event": FormTemplate(
        "vol-event",
        "volunteer",
        "event",
        "Event",
        "Owns the event concept and ensures the rundown runs as planned from opening to closing.",
        60,
    ),
    "vol-runner": FormTemplate(
        "vol-runner",
        "volunteer",
        "runner",
        "Runner",
        "Supports audience Q&A microphone flow and helps speakers receive stage equipment on time.",
        70,
    ),
    "vol-social-media": FormTemplate(
        "vol-social-media",
        "volunteer",
        "social-media",
        "Social Media",
        "Live-posts event updates and highlights to AWS UG Jakarta social channels.",
        80,
    ),
    "vol-liaison-officer": FormTemplate(
        "vol-liaison-officer",
        "volunteer",
        "liaison-officer",
        "Liaison Officer",
        "Supports speakers before, during, and after events by coordinating schedules and presenter needs.",
        90,
    ),
    "vol-sponsorship": FormTemplate(
        "vol-sponsorship",
        "volunteer",
        "sponsorship",
        "Sponsorship",
        "Builds relationships with sponsors and partners to secure event funding, resources, and in-kind support.",
        100,
    ),
    "vol-moderator-mc": FormTemplate(
        "vol-moderator-mc",
        "volunteer",
        "moderator-mc",
        "Moderator / MC",
        "Guides the event flow, keeps energy high, and hosts audience engagement or ice-breaking sessions.",
        110,
    ),
    "vol-website": FormTemplate(
        "vol-website",
        "volunteer",
        "website",
        "Website",
        "Builds and maintains the AWS UG Jakarta website, keeping event information and community resources up to date.",
        120,
    ),
    "speaker-main": FormTemplate(
        "speaker-main",
        "speaker",
        "speaker",
        "Speaker Application",
        "Apply once as a speaker. Choose your preferred talk format inside the application form.",
        10,
    ),
}

EMAIL_HINTS = ("email", "e-mail", "mail address", "alamat email")
LINKEDIN_HINTS = ("linkedin", "linked in")


def default_env_path() -> str:
    return os.path.join(os.path.dirname(__file__), ".env")


def load_env_file(path: str) -> None:
    if not os.path.exists(path):
        return

    with open(path, "r", encoding="utf-8") as file:
        for line_number, raw_line in enumerate(file, start=1):
            line = raw_line.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith("export "):
                line = line.removeprefix("export ").strip()
            if "=" not in line:
                raise ValueError(
                    f"Invalid env line {line_number} in {path}: missing '='"
                )

            key, value = line.split("=", 1)
            key = key.strip()
            value = value.strip()
            if not key:
                raise ValueError(f"Invalid env line {line_number} in {path}: empty key")

            parsed = shlex.split(value, comments=False, posix=True)
            os.environ.setdefault(key, parsed[0] if parsed else "")


def main() -> int:
    load_env_file(default_env_path())

    parser = argparse.ArgumentParser(
        description="Discover Formbricks question IDs and generate application_forms SQL."
    )
    parser.add_argument("--mapper", required=True, help="Path to mapper JSON file.")
    parser.add_argument(
        "--output",
        default="ingest_mapper_formbrick.generated.sql",
        help="Output SQL path.",
    )
    parser.add_argument(
        "--base-url",
        default=os.environ.get("FORMBRICKS_BASE_URL", "https://forms.awscommunity.id"),
        help="Formbricks base URL. Defaults to FORMBRICKS_BASE_URL.",
    )
    parser.add_argument(
        "--api-key",
        default=os.environ.get("FORMBRICKS_API_KEY"),
        help="Formbricks Management API key. Defaults to FORMBRICKS_API_KEY.",
    )
    parser.add_argument(
        "--archive-after-sql",
        default=os.environ.get("ARCHIVE_AFTER_SQL", "datetime('now', '+6 months')"),
        help="SQL expression for archive_after. Use NULL to disable. Defaults to datetime('now', '+6 months').",
    )
    args = parser.parse_args()

    if not args.api_key:
        print("FORMBRICKS_API_KEY is required.", file=sys.stderr)
        return 2

    mapper = load_mapper(args.mapper)
    rows: list[str] = []
    errors: list[str] = []

    for form_id, public_url in mapper.items():
        template = FORM_TEMPLATES.get(form_id)
        if not template:
            errors.append(f"Unknown form id: {form_id}")
            continue

        survey_id = extract_survey_id(public_url)
        if not survey_id:
            errors.append(
                f"Cannot extract survey id from URL for {form_id}: {public_url}"
            )
            continue
        if survey_id.startswith("REPLACE_"):
            errors.append(
                f"{form_id}: survey URL still contains placeholder {survey_id}"
            )
            continue

        try:
            survey = fetch_survey(args.base_url, args.api_key, survey_id)
            questions = extract_questions(survey)
            email_question_id = find_question_id(questions, EMAIL_HINTS)
            linkedin_question_id = find_question_id(questions, LINKEDIN_HINTS)
        except Exception as exc:  # noqa: BLE001 - CLI should report and continue.
            errors.append(f"{form_id}: {exc}")
            continue

        if not email_question_id:
            errors.append(f"{form_id}: email question ID not found")
            continue
        if not linkedin_question_id:
            errors.append(f"{form_id}: LinkedIn question ID not found")
            continue
        if email_question_id == linkedin_question_id:
            errors.append(
                f"{form_id}: email and LinkedIn resolved to same question "
                f"({email_question_id}); question text may lack distinguishing hints"
            )
            continue

        rows.append(
            render_row(
                template,
                survey_id,
                public_url,
                email_question_id,
                linkedin_question_id,
                args.archive_after_sql,
            )
        )

    if errors:
        print("Discovery errors:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    sql = render_sql(rows)
    with open(args.output, "w", encoding="utf-8") as file:
        file.write(sql)

    print(f"Generated {args.output} with {len(rows)} form mappings.")
    return 0


def load_mapper(path: str) -> dict[str, str]:
    with open(path, "r", encoding="utf-8") as file:
        data = json.load(file)

    if not isinstance(data, dict):
        raise ValueError("Mapper must be a JSON object of { form_id: public_url }.")

    return {str(key): str(value) for key, value in data.items()}


def extract_survey_id(public_url: str) -> str | None:
    parsed = urllib.parse.urlparse(public_url)
    parts = [part for part in parsed.path.split("/") if part]
    if "s" in parts:
        index = parts.index("s")
        if index + 1 < len(parts):
            return parts[index + 1]
    if parts:
        return parts[-1]
    return None


def fetch_survey(base_url: str, api_key: str, survey_id: str) -> dict[str, Any]:
    base = base_url.rstrip("/")
    paths = (
        f"/api/v1/management/surveys/{survey_id}",
        f"/api/v2/management/surveys/{survey_id}",
        f"/api/v1/surveys/{survey_id}",
    )
    last_error = ""

    for path in paths:
        url = f"{base}{path}"
        request = urllib.request.Request(
            url,
            headers={
                "x-api-key": api_key,
                "Authorization": f"Bearer {api_key}",
                "Accept": "application/json",
                "User-Agent": (
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                    "(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"
                ),
                "Referer": f"{base}/",
            },
            method="GET",
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace")
            last_error = f"{url} returned {exc.code}: {body[:300]}"
            if exc.code == 404:
                continue
        except urllib.error.URLError as exc:
            last_error = f"{url} failed: {exc}"

    raise RuntimeError(f"Unable to fetch survey {survey_id}. {last_error}")


def extract_questions(node: Any) -> list[dict[str, Any]]:
    questions: list[dict[str, Any]] = []

    def walk(value: Any) -> None:
        if isinstance(value, dict):
            if isinstance(value.get("id"), str) and looks_like_question(value):
                questions.append(value)
            for child in value.values():
                walk(child)
        elif isinstance(value, list):
            for child in value:
                walk(child)

    walk(node)
    unique: dict[str, dict[str, Any]] = {}
    for question in questions:
        unique.setdefault(question["id"], question)
    return list(unique.values())


def looks_like_question(value: dict[str, Any]) -> bool:
    question_keys = {
        "headline",
        "subheader",
        "html",
        "type",
        "inputType",
        "placeholder",
        "required",
        "choices",
    }
    return any(key in value for key in question_keys)


def find_question_id(
    questions: list[dict[str, Any]], hints: tuple[str, ...]
) -> str | None:
    scored: list[tuple[int, str]] = []

    for question in questions:
        text = normalize_question_text(question)
        score = 0
        for hint in hints:
            if hint in text:
                score += 10
        if "headline" in question:
            score += 2
        if score > 0:
            scored.append((score, question["id"]))

    if not scored:
        return None

    scored.sort(reverse=True)
    return scored[0][1]


def normalize_question_text(question: dict[str, Any]) -> str:
    # Only use text-oriented keys — 'type' and 'inputType' are structural
    # metadata (e.g. "openText", "email", "url") and would cause false
    # matches against EMAIL_HINTS / LINKEDIN_HINTS.
    chunks: list[str] = []
    for key in (
        "headline",
        "subheader",
        "html",
        "label",
        "placeholder",
        "name",
    ):
        value = question.get(key)
        if isinstance(value, str):
            chunks.append(value)
        elif isinstance(value, dict):
            chunks.extend(str(v) for v in value.values() if isinstance(v, str))
    return strip_html(" ".join(chunks)).lower()


def strip_html(value: str) -> str:
    return re.sub(r"<[^>]+>", " ", value)


def sql_string(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def render_row(
    template: FormTemplate,
    survey_id: str,
    public_url: str,
    email_question_id: str,
    linkedin_question_id: str,
    archive_after_sql: str,
) -> str:
    values = [
        sql_string(template.id),
        sql_string(template.kind),
        sql_string(template.slug),
        sql_string(template.title),
        sql_string(template.description),
        sql_string(survey_id),
        sql_string(public_url),
        sql_string(email_question_id),
        sql_string(linkedin_question_id),
        "1",
        "NULL",
        "NULL",
        "NULL",
        archive_after_sql,
        str(template.display_order),
    ]
    return "(\n  " + ",\n  ".join(values) + "\n)"


def render_sql(rows: list[str]) -> str:
    if not rows:
        raise ValueError("No rows generated.")

    return (
        "INSERT INTO application_forms (\n"
        "  id,\n"
        "  kind,\n"
        "  slug,\n"
        "  title,\n"
        "  description,\n"
        "  formbricks_survey_id,\n"
        "  formbricks_public_url,\n"
        "  email_question_id,\n"
        "  linkedin_question_id,\n"
        "  is_active,\n"
        "  opens_at,\n"
        "  closes_at,\n"
        "  editable_until,\n"
        "  archive_after,\n"
        "  display_order\n"
        ") VALUES\n" + ",\n".join(rows) + "\nON CONFLICT(kind, slug) DO UPDATE SET\n"
        "  title = excluded.title,\n"
        "  description = excluded.description,\n"
        "  formbricks_survey_id = excluded.formbricks_survey_id,\n"
        "  formbricks_public_url = excluded.formbricks_public_url,\n"
        "  email_question_id = excluded.email_question_id,\n"
        "  linkedin_question_id = excluded.linkedin_question_id,\n"
        "  is_active = excluded.is_active,\n"
        "  opens_at = excluded.opens_at,\n"
        "  closes_at = excluded.closes_at,\n"
        "  editable_until = excluded.editable_until,\n"
        "  archive_after = excluded.archive_after,\n"
        "  display_order = excluded.display_order,\n"
        "  updated_at = datetime('now');\n"
    )


if __name__ == "__main__":
    raise SystemExit(main())
