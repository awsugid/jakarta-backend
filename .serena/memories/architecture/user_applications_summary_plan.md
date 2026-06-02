# User Applications Summary — Backend Plan

## Feature Overview

Authenticated users can view all of their submitted applications across all active forms (volunteer + speaker). Backend provides a summary endpoint that aggregates discovery results, plus a response detail endpoint and an edit link endpoint with prefill support.

## New Endpoints

### `GET /api/applications/summary`

Returns all active forms where the authenticated user has an existing response. Auth required.

**Processing:**
1. Extract and validate Google auth token → get normalized `user_email`.
2. Query D1: `SELECT * FROM application_forms WHERE is_active = 1 ORDER BY kind, display_order`.
3. For each form row, run the existing FormBricks email discovery scan (`formbricks_survey_id` + `email_question_id`).
4. Collect only rows where a matching response is found.
5. For each match, check policy: `editable_until` → compute `editable: bool`.
6. Return aggregated list.

**Response shape:**
```json
[
  {
    "kind": "volunteer",
    "slug": "foh",
    "title": "FOH (Front of House)",
    "description": "...",
    "response_id": "<formbricks-response-id>",
    "finished": true,
    "editable": true,
    "submitted_at": "2026-05-01T10:00:00Z"
  }
]
```

**Performance note:** This endpoint runs one FormBricks scan per active form. If there are many active forms and many responses, this can be slow. Acceptable for phase 1 (community event, low volume). If slow, cache result per user in KV with a short TTL (e.g. 60s). Do not add caching unless proven necessary.

---

### `GET /api/applications/:kind/:slug/response`

Returns the full answer data for the user's existing response. Auth required.

**Processing:**
1. Validate auth → `user_email`.
2. Load `application_forms` row by `(kind, slug)`.
3. Run email discovery scan → get `response_id`.
4. If no response found: return 404.
5. Fetch full response from FormBricks: `GET /api/v2/management/responses/:response_id`.
6. Return response `data` map (question_id → answer) + form metadata.

**Response shape:**
```json
{
  "form": {
    "kind": "volunteer",
    "slug": "foh",
    "title": "FOH (Front of House)"
  },
  "response": {
    "id": "<response-id>",
    "finished": true,
    "submitted_at": "2026-05-01T10:00:00Z",
    "data": {
      "<question_id_1>": "answer text",
      "<question_id_2>": "linkedin.com/in/johndoe"
    }
  }
}
```

Frontend uses this to render a read-only view of submitted answers.

---

### `POST /api/applications/:kind/:slug/link` (extended — edit mode)

Existing endpoint extended with optional `mode=edit` query param or `{ "mode": "edit" }` body.

**Edit mode processing:**
1. Validate auth → `user_email`.
2. Load form row, check `editable_until` policy → if past, return 403 with `code: "not_editable"`.
3. Run email discovery → find existing `response_id`.
4. If no existing response: return 404 (use normal link flow for new applications).
5. Fetch full response data from FormBricks (`GET /api/v2/management/responses/:response_id`).
6. Build prefilled FormBricks URL:
   ```
   <formbricks_public_url>?<question_id_1>=<url_encoded_answer_1>&<question_id_2>=<url_encoded_answer_2>&skipPrefilled=true
   ```
7. Return `{ url, editable: true, response_id }`.

**Old response cleanup:** When user re-submits via prefilled iframe, FormBricks creates a new response. The `responseFinished` webhook fires. Backend detects same email as existing response → deletes old response. This is handled by existing webhook logic in `mem:architecture/iframe_webhook_plan` — no changes needed there.

---

## Updated Full Endpoint List

```text
GET  /health
GET  /api/forms
GET  /api/forms/:kind
GET  /api/forms/:kind/:slug
GET  /api/applications/summary                        ← NEW
GET  /api/applications/:kind/:slug
GET  /api/applications/:kind/:slug/response           ← NEW
POST /api/applications/:kind/:slug/validate
POST /api/applications/:kind/:slug/link               ← extended (edit mode)
POST /api/webhook/formbricks                          ← from iframe_webhook_plan
```

**Route ordering note:** `/api/applications/summary` must be registered before `/api/applications/:kind/:slug` in the router to avoid `:kind` matching the literal string "summary".

---

## New FormBricks Client Methods

Add to `src/formbricks/client.rs`:

```rust
// Fetch a single response by ID
async fn get_response(&self, response_id: &str) -> Result<FormBricksResponse, WorkerError>
// GET /api/v2/management/responses/{id}
```

Already planned (from `mem:architecture/formbricks_application_plan`):
- `list_responses(survey_id, limit, skip)` — reused for discovery scan in summary.
- `delete_response(response_id)` — reused by webhook handler.

---

## Policy Enforcement

`editable` field computation in `GET /api/applications/summary` and edit mode of `/link`:

```rust
fn is_editable(form: &ApplicationForm) -> bool {
    match &form.editable_until {
        None => true, // no expiry set = always editable
        Some(dt) => Utc::now() < *dt,
    }
}
```

Also check `is_active`, `closes_at` — a closed form is not editable even if `editable_until` is in the future.

---

## Error Codes

New error codes to add to `src/http/errors.rs`:

| Code | HTTP Status | Meaning |
|---|---|---|
| `not_editable` | 403 | `editable_until` has passed |
| `response_not_found` | 404 | No existing response for this user+form |
| `form_not_found` | 404 | No `application_forms` row for (kind, slug) |

---

## Implementation Sequence

1. Add `get_response(response_id)` to `src/formbricks/client.rs`.
2. Add `response_not_found` and `not_editable` error codes to `src/http/errors.rs`.
3. Implement summary aggregation logic in `src/application/service.rs`:
   - `list_user_applications(email) -> Vec<UserApplication>`.
   - Reuses existing `discover_application(form, email)` per form.
4. Add `GET /api/applications/summary` route handler in `src/http/routes.rs`.
   - Register before `/:kind/:slug` pattern.
5. Implement `GET /api/applications/:kind/:slug/response` route handler.
6. Extend `POST /api/applications/:kind/:slug/link` to support edit mode:
   - Accept `?mode=edit` query param.
   - Build prefilled URL from existing response data.
7. Add `is_editable` policy check in `src/application/policy.rs`.

## Key Risks

- Summary endpoint scans FormBricks for every active form on each call — O(forms × responses). Add KV cache per user if latency is unacceptable.
- Prefilled URL for edit may expose answer data in browser history/logs — acceptable for community form, not for sensitive data.
- `summary` literal must not conflict with `:kind` route param — register fixed routes before parameterized ones.
- If user submits the edit form but webhook fails to delete old response, two responses exist for same email. Webhook retry from FormBricks should handle this; log and alert if both persist after 5 minutes.
