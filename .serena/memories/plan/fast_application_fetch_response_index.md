# Fast Application Fetch Plan — D1 Response Index

## Problem

`GET /api/applications/summary` is slow because it currently:

1. Loads every active `application_forms` row.
2. For each form, calls `discover_user_application`.
3. `discover_user_application` paginates Formbricks responses and searches answers by normalized email.

This is `O(active_forms × Formbricks_response_pages)` per user request. The webhook duplicate check has the same issue because it scans Formbricks responses again to compare LinkedIn URLs.

## Decision

Move from request-time discovery to webhook-maintained indexing.

D1 should store a small searchable index of submitted Formbricks responses. Formbricks remains the source of truth for full response data. D1 becomes the fast lookup/index layer for:

- user application summary by normalized email,
- existing application lookup by `(form_id, normalized_email)`,
- duplicate LinkedIn detection by `(form_id, normalized_linkedin_url)`.

This was previously marked as the later-phase fallback in `mem:architecture/formbricks_application_plan`; current latency proves it is now needed.

## Sources Considered

1. Live Formbricks scan per active form — current implementation; slow.
2. Short TTL KV cache per user — helps repeat calls but first call remains slow and cache invalidation is weak.
3. D1 response index maintained by Formbricks webhooks — best balance: fast, small schema, reactive, minimal API changes.
4. Backend-mediated/headless submission — stronger control but larger product/API change.
5. Formbricks unique constraints — not confirmed for arbitrary question-answer uniqueness; not reliable for this use case.
6. Periodic backfill only — useful for migration, but stale without webhooks.
7. Store full responses in D1 — overkill; duplicates Formbricks source of truth.

## Target Schema

Add migration `0002_application_response_index.sql`:

```sql
CREATE TABLE IF NOT EXISTS application_response_index (
  id TEXT PRIMARY KEY,
  form_id TEXT NOT NULL REFERENCES application_forms(id),
  formbricks_survey_id TEXT NOT NULL,
  formbricks_response_id TEXT NOT NULL,
  normalized_email TEXT NOT NULL,
  normalized_linkedin_url TEXT,
  finished INTEGER NOT NULL DEFAULT 1,
  status TEXT NOT NULL DEFAULT 'active',
  submitted_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(form_id, formbricks_response_id),
  UNIQUE(form_id, normalized_email),
  UNIQUE(form_id, normalized_linkedin_url)
);

CREATE INDEX IF NOT EXISTS idx_response_index_email
  ON application_response_index(normalized_email, status);

CREATE INDEX IF NOT EXISTS idx_response_index_form_email
  ON application_response_index(form_id, normalized_email, status);

CREATE INDEX IF NOT EXISTS idx_response_index_form_linkedin
  ON application_response_index(form_id, normalized_linkedin_url, status);
```

Notes:

- `normalized_linkedin_url` should be nullable only if extraction/normalization fails. Normal valid submissions should always have it.
- Unique LinkedIn should ignore null behavior naturally in SQLite/D1.
- `status` allows future values: `active`, `duplicate_deleted`, `deleted`, `archived`.

## Webhook Behavior

Update `src/http/webhook.rs` after signature validation and form lookup:

1. Process `responseFinished` with `finished = true`.
2. Extract email and LinkedIn using stored `email_question_id` and `linkedin_question_id`.
3. Normalize email and LinkedIn with existing validation utilities.
4. Check D1 index for existing row with same `(form_id, normalized_linkedin_url)`.
5. If duplicate LinkedIn exists and belongs to another normalized email:
   - delete current Formbricks response via Management API,
   - insert/update index row for current response with `status = 'duplicate_deleted'` or skip indexing it,
   - log enough context: survey ID, response ID, form ID, normalized email, duplicate response ID.
6. If same email re-submits/edit flow:
   - replace old `(form_id, normalized_email)` row with new response ID,
   - delete old Formbricks response if current edit cleanup behavior requires it,
   - keep index pointing to the latest active response.
7. If no duplicate:
   - upsert active row into `application_response_index`.

This removes the webhook’s current `get_all_responses` scan.

## Application Fetch Behavior

Update summary and detail discovery paths:

### `GET /api/applications/summary`

Replace per-form Formbricks scans with one indexed query:

```sql
SELECT
  f.kind,
  f.slug,
  f.title,
  f.description,
  i.formbricks_response_id,
  i.finished,
  i.submitted_at,
  f.editable_until,
  f.closes_at,
  f.is_active
FROM application_response_index i
JOIN application_forms f ON f.id = i.form_id
WHERE i.normalized_email = ?
  AND i.status = 'active'
  AND f.is_active = 1
ORDER BY f.kind, f.display_order, f.title;
```

Then compute `editable` with existing policy logic.

### `GET /api/applications/:kind/:slug`

Load form by `(kind, slug)`, then lookup:

```sql
SELECT * FROM application_response_index
WHERE form_id = ?
  AND normalized_email = ?
  AND status = 'active'
LIMIT 1;
```

Return `exists` from D1. Do not scan Formbricks.

### `GET /api/applications/:kind/:slug/response`

Use D1 to find `response_id`, then call Formbricks `get_response(response_id)` only for full response data. This is one direct lookup, not a scan.

### `POST /api/applications/:kind/:slug/validate`

Check duplicate LinkedIn in D1 by `(form_id, normalized_linkedin_url)` instead of scanning Formbricks.

### `POST /api/applications/:kind/:slug/link?mode=edit`

Use D1 to find the user’s active response ID. Fetch full Formbricks response only after D1 confirms it exists.

## Backfill Plan

Because existing submissions are already in Formbricks, add a one-time backfill before relying on the index:

1. Create migration for `application_response_index`.
2. Add an admin-only/manual backfill script or temporary Worker route.
3. For each active form:
   - fetch Formbricks responses with current `get_all_responses`,
   - extract/normalize email and LinkedIn,
   - insert active index row for finished responses,
   - log skipped rows with missing/invalid email or LinkedIn.
4. Run once after deployment.
5. Verify counts per form between Formbricks and D1 index.
6. Remove or protect the backfill route if a route was used.

## Implementation Order

1. Add D1 migration for `application_response_index`.
2. Extend `FormRepository` with index methods:
   - `get_index_by_form_email(form_id, normalized_email)`,
   - `list_indexes_by_email(normalized_email)`,
   - `get_index_by_form_linkedin(form_id, normalized_linkedin_url)`,
   - `upsert_active_response_index(...)`,
   - `mark_response_index_status(response_id, status)`.
3. Update webhook to write/read the index and remove Formbricks scan.
4. Update application service discovery/summary/validate/link flows to use D1 index.
5. Keep Formbricks `get_response(response_id)` only for full response detail and edit prefill.
6. Add one-time backfill utility.
7. Validate with `cargo fmt`, `cargo check`, and focused tests if available.

## Logging To Validate Assumptions Before Full Cutover

Add logs around the current slow paths first or during rollout:

- number of active forms scanned in summary,
- per-form Formbricks response count/pages fetched,
- total summary latency,
- webhook duplicate check latency,
- index hit/miss for email lookup,
- index hit/miss for LinkedIn duplicate lookup.

These logs prove the bottleneck and confirm the index is being used.

## Expected Performance Change

Before:

- Summary: `N active forms × paginated Formbricks list responses`.
- Duplicate check: paginated Formbricks list responses.

After:

- Summary: one D1 indexed join query.
- Existing application check: one D1 indexed lookup.
- Duplicate LinkedIn check: one D1 indexed lookup.
- Full response detail/edit: one D1 lookup + one direct Formbricks response fetch.

## Risks

- Webhook delivery failure can make D1 stale. Mitigation: Formbricks retries + manual backfill/reconciliation.
- Existing records need backfill or users will appear to have no applications.
- Unique `(form_id, normalized_email)` means edit re-submission must replace the active row cleanly.
- Unique `(form_id, normalized_linkedin_url)` means duplicate handling must mark/delete old conflicting rows before inserting new active rows.
- If admins change `email_question_id` or `linkedin_question_id`, backfill/reconciliation may be needed.

## Recommendation

Implement the D1 response index now. KV caching is not enough because it only masks repeat requests and does not fix duplicate validation/webhook scans. The index is the smallest reliable change that makes fetching applications fast while keeping Formbricks as the source of truth for full response content.
