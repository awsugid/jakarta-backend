# IFrame + Webhook Implementation Plan

## Chosen Approach

Render the FormBricks form via iframe embed. Validate email pre-submission (preventive) and LinkedIn post-submission (reactive via webhook). This is the chosen approach over headless `survey-ui` rendering due to lower build complexity and acceptable reactive gap for community event use case.

## Full User Flow

```
1. User visits /volunteer or /speakers page
   → frontend calls GET /api/forms/:kind to load active form cards

2. User clicks "Apply"
   → check Google auth; prompt Sign-In if not authenticated

3. Frontend calls GET /api/applications/:kind/:slug
   Authorization: Bearer <google-id-token>
   → exists: true  → show existing application status (editable or view-only), stop
   → exists: false → proceed

4. Frontend calls POST /api/applications/:kind/:slug/link
   → backend checks form is_active / opens_at / closes_at policy
   → returns { url: "<formbricks-public-url>", editable: bool }

5. Frontend opens FormBricks iframe using the returned URL
   → user fills and submits the form inside iframe
   → FormBricks saves the response

6. FormBricks fires responseFinished webhook to backend
   POST /api/webhook/formbricks
   → backend extracts linkedin_url from response.data[linkedin_question_id]
   → backend normalizes linkedin_url
   → backend scans existing responses for duplicate normalized LinkedIn
     (excluding the current response by response id)
   → duplicate found: DELETE /api/v2/management/responses/{id} (remove the new duplicate)
   → no duplicate: response is accepted, flow complete
```

## Email Duplicate Prevention (Preventive)

- Handled at step 3 via existing `GET /api/applications/:kind/:slug` endpoint.
- Scans FormBricks responses for exact normalized email match against Google auth email.
- If match found, user is shown their existing application and the iframe is never opened.
- No changes needed to existing planned endpoint.

## LinkedIn Duplicate Prevention (Reactive via Webhook)

- Triggered after FormBricks saves the response (`responseFinished` event).
- Reactive gap: response exists for seconds before backend processes the webhook.
- Acceptable trade-off for community event volume and low-stakes context.
- Race condition risk (two users submitting same LinkedIn simultaneously) is low-probability at community meetup scale.

## Webhook Endpoint

### Route
```
POST /api/webhook/formbricks
```

### Security
- FormBricks sends a webhook signing secret in the request header.
- Backend must verify the signature before processing.
- Store webhook signing secret as a Cloudflare Worker secret: `FORMBRICKS_WEBHOOK_SECRET`.
- Reject requests with invalid or missing signatures with HTTP 401.

### Events to Handle
- `responseFinished` — primary event; full response data with `finished: true`.
- Ignore `responseCreated` and `responseUpdated` — incomplete responses, not worth checking yet.

### Webhook Payload Shape (relevant fields)
```json
{
  "event": "responseFinished",
  "webhookId": "...",
  "data": {
    "id": "<response-id>",
    "surveyId": "<formbricks-survey-id>",
    "finished": true,
    "data": {
      "<email_question_id>": "user@example.com",
      "<linkedin_question_id>": "https://linkedin.com/in/johndoe"
    }
  }
}
```

### Processing Logic
1. Parse and verify webhook signature.
2. Check `event == "responseFinished"` and `data.finished == true`; ignore otherwise.
3. Look up `application_forms` row by `formbricks_survey_id` from `data.surveyId`.
   - If no row found: log and return 200 (unknown survey, not our concern).
4. Extract `data.data[linkedin_question_id]` → normalize LinkedIn URL.
5. Extract `data.data[email_question_id]` → normalize email.
6. Query FormBricks Management API responses for this `surveyId`.
7. Paginate responses; for each response (excluding current `data.id`):
   - Normalize `response.data[linkedin_question_id]`.
   - If normalized LinkedIn matches → duplicate found.
8. On duplicate found:
   - Call `DELETE /api/v2/management/responses/{data.id}` to remove the new response.
   - Log the event with survey_id, response_id, and masked LinkedIn slug for audit.
9. Return HTTP 200 regardless of outcome (FormBricks expects 2xx to stop retries).

### Error Handling
- If FormBricks delete API fails: log the error with full context, return 200 to FormBricks (avoid retry storm), flag for manual review.
- If pagination limit hit during scan: log a warning, do not delete (safer than false positive deletion).
- Never return 4xx/5xx to FormBricks webhook unless signature verification fails — FormBricks will retry on non-2xx.

## New Backend Code

### New Module: `src/http/webhook.rs`
Handles `POST /api/webhook/formbricks` route.

### New Worker Secret
```
FORMBRICKS_WEBHOOK_SECRET
```
Add to `wrangler.toml` under `[vars]` reference and provision via `wrangler secret put`.

### FormBricks Client Addition (`src/formbricks/client.rs`)
Add:
- `delete_response(response_id)` → `DELETE /api/v2/management/responses/{id}`
- Already planned: `list_responses(survey_id, limit, skip)` — reused for duplicate scan.

### Route Registration (`src/http/routes.rs`)
Add:
```
POST /api/webhook/formbricks  → webhook::handle
```

## Frontend Changes

### Iframe Embed
- After receiving `{ url }` from `POST /api/applications/:kind/:slug/link`, render:
```tsx
<iframe
  src={url}
  className="w-full h-[600px] border-0 rounded-lg"
  allow="clipboard-write"
/>
```
- Wrap in a modal or dedicated section consistent with existing site design.
- No `@formbricks/survey-ui` package needed.

### Post-submission UX
- No real-time feedback from FormBricks iframe to parent page on submission (cross-origin iframe).
- Show a "Thank you — we'll confirm your application shortly" message after a fixed timeout or on modal close.
- If LinkedIn duplicate is detected by webhook, send an email notification to the user (future phase — not in scope now).

## Out of Scope (Phase 1)

- Email notification to user when LinkedIn duplicate is detected and their response is deleted.
- Webhook-backed `application_response_index` table in D1 for faster scanning.
- Admin UI for managing `application_forms` records.
- `responseCreated` / `responseUpdated` event handling.

## Implementation Sequence

1. Add `FORMBRICKS_WEBHOOK_SECRET` to Worker secrets and `wrangler.toml`.
2. Add `delete_response` to `src/formbricks/client.rs`.
3. Implement `src/http/webhook.rs`:
   - Signature verification.
   - Event filter (`responseFinished` only).
   - Survey lookup from D1.
   - LinkedIn extraction + normalization (reuse existing `validation/linkedin.rs`).
   - Duplicate scan via `formbricks/client.rs` `list_responses`.
   - Delete on duplicate.
4. Register route in `src/http/routes.rs`.
5. Frontend: implement Google Sign-In flow.
6. Frontend: call `/api/applications/:kind/:slug` on Apply click.
7. Frontend: call `/api/applications/:kind/:slug/link` to get iframe URL.
8. Frontend: render iframe in modal/section.
9. Configure FormBricks webhook to point to `POST /api/webhook/formbricks` with signing secret.

## Key Risks

- Reactive gap: LinkedIn duplicate response exists briefly before webhook fires and deletes it. Acceptable for community event scale.
- Webhook delivery failure: response stays if Cloudflare Worker is down or returns error. Mitigate by always returning 200 and logging for manual review.
- FormBricks pagination during scan: if survey has many responses, scan may be slow. Add configurable page limit and log warnings at threshold.
- Cross-origin iframe: no `postMessage` events from FormBricks to parent page. Post-submission UX relies on timeout or user action, not a submit event.
