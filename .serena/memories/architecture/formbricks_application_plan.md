# FormBricks Application Architecture Plan

## Product Context

- Backend: Rust Cloudflare Worker for jakarta.awscommunity.id.
- FormBricks: self-hosted; base URL must be configurable, do not hardcode `app.formbricks.com`.
- Frontend page has volunteer division cards such as Registration, FOH, Logistics, Design, Documentation, Event. Each card is a separate application target and should open/use a dedicated FormBricks form/survey.
- Speaker application behaves similarly to volunteer applications, but without division card semantics unless later added.
- Primary requirement: admins must be able to change form fields/questions in FormBricks without deploying frontend code.
- Backend acts as proxy/orchestration layer over FormBricks APIs; frontend must not receive FormBricks Management API key.

## Final Architecture Decision

- FormBricks is the source of truth for form structure and response content.
- Backend D1 is not the primary application data store in phase 1.
- D1 is used as a form registry and policy store only:
  - which application forms exist,
  - which site card/slug points to which FormBricks survey,
  - which FormBricks question id contains Email Address,
  - which FormBricks question id contains LinkedIn Profile URL,
  - open/close/editable/archive policy.
- Application discovery is performed live against FormBricks responses by exact normalized email match for the selected survey/form.
- Duplicate prevention is performed by normalized LinkedIn Profile URL match within the selected survey/form.
- If live scanning FormBricks responses becomes too slow, add a response index table later via webhooks. Do not start with this unless necessary.

## Identity and Duplicate Rules

- Google SSO identifies the current browser user.
- Current user's existing application is discovered by matching the FormBricks response's Email Address answer to the Google email.
- Submitted email must match Google email after normalization.
- LinkedIn Profile URL is the duplicate-applicant key.
- Email is not sufficient for duplicate prevention because applicants can use aliases or alternate email addresses.
- LinkedIn URL is not sufficient for authentication because another user could paste someone else's LinkedIn URL. Use it for duplicate prevention only.

### Normalization

- Email normalization: `trim().to_lowercase()`.
- Do not remove dots or plus aliases unless explicitly requested later; Gmail-specific normalization can surprise users.
- LinkedIn URL normalization:
  - accept only public profile paths like `linkedin.com/in/{slug}` or `www.linkedin.com/in/{slug}`,
  - strip scheme (`http://`, `https://`), query string, fragment, trailing slash,
  - lowercase host and path slug unless product later needs case-sensitive preservation,
  - canonical output example: `linkedin.com/in/johndoe`.
- Reject non-profile LinkedIn URLs such as `/company`, `/jobs`, `/feed`, `/school`, or arbitrary domains.

## FormBricks Capabilities and Constraints

- FormBricks can host/edit forms and store responses.
- FormBricks Management API exposes responses APIs, contacts APIs, survey/contact links, etc.
- Relevant endpoints from docs:
  - `GET /api/v2/management/responses` with `surveyId`, pagination, sorting filters.
  - `POST /api/v2/management/responses` to create responses if backend-mediated/headless mode is used later.
  - `PUT /api/v2/management/responses/{id}` to update responses if backend-mediated/headless mode is used later.
  - `GET /api/v2/management/surveys/{surveyId}/contact-links/contacts/{contactId}` if using contact-personalized links later.
  - `POST /api/v2/management/contacts` if contact management becomes part of the flow later.
- FormBricks has related features but no confirmed built-in unique-value constraint for “reject response if LinkedIn URL already exists in this survey”.
- FormBricks can validate field shape via validation/regex; use this for LinkedIn URL format where practical, but uniqueness belongs in backend.
- FormBricks Limit Submissions closes a whole survey after a total count; it does not solve unique LinkedIn duplicate prevention.
- FormBricks Quota Management is Enterprise and quota-oriented, not a simple per-value uniqueness constraint.
- Single-use links prevent reuse of the same link, but do not prevent duplicate LinkedIn values across different links.

## Phase 1 D1 Schema

Use D1 only for application form metadata/policy.

```sql
create table application_forms (
  id text primary key,
  kind text not null check (kind in ('volunteer', 'speaker')),
  slug text not null,
  title text not null,
  description text,
  formbricks_survey_id text not null,
  formbricks_public_url text,
  email_question_id text not null,
  linkedin_question_id text not null,
  is_active integer not null default 1,
  opens_at text,
  closes_at text,
  editable_until text,
  archive_after text,
  display_order integer not null default 0,
  created_at text not null,
  updated_at text not null,
  unique(kind, slug)
);

create index idx_application_forms_kind_active
  on application_forms(kind, is_active, display_order);
```

Optional later phase table if FormBricks response scanning is too slow:

```sql
create table application_response_index (
  id text primary key,
  form_id text not null references application_forms(id),
  formbricks_survey_id text not null,
  formbricks_response_id text not null,
  normalized_email text not null,
  normalized_linkedin_url text,
  status text not null,
  finished integer,
  created_at text not null,
  updated_at text not null,
  unique(form_id, normalized_email),
  unique(form_id, normalized_linkedin_url)
);
```

Do not add `application_response_index` in phase 1 unless implementing webhook sync or there is a proven performance problem.

## Backend API Contract

Suggested public API exposed by Worker:

```text
GET  /health
GET  /api/forms
GET  /api/forms/:kind
GET  /api/forms/:kind/:slug
GET  /api/applications/:kind/:slug
POST /api/applications/:kind/:slug/validate
POST /api/applications/:kind/:slug/link
```

Meanings:

- `GET /api/forms` returns active application cards/forms with metadata needed by frontend.
- `GET /api/forms/:kind` returns forms for volunteer or speaker.
- `GET /api/forms/:kind/:slug` returns one registered form and policy status.
- `GET /api/applications/:kind/:slug` requires Google auth; discovers whether the current user already has a response in that FormBricks survey by matching email answer.
- `POST /api/applications/:kind/:slug/validate` requires Google auth; validates a proposed submitted email and LinkedIn URL against Google email and existing FormBricks responses. Use before redirecting/opening form when possible, or from webhook/headless flow later.
- `POST /api/applications/:kind/:slug/link` requires Google auth; returns the FormBricks link/embed URL only if form is active/editable and current user is allowed to continue.

Example `GET /api/applications/volunteer/foh` response:

```json
{
  "form": {
    "kind": "volunteer",
    "slug": "foh",
    "title": "FOH (Front of House)",
    "surveyId": "...",
    "isActive": true,
    "editableUntil": "2026-07-01T00:00:00Z"
  },
  "application": {
    "exists": true,
    "responseId": "...",
    "finished": true,
    "submittedEmail": "user@example.com",
    "linkedinUrl": "linkedin.com/in/example",
    "editable": true
  }
}
```

Example duplicate validation response:

```json
{
  "ok": false,
  "code": "duplicate_linkedin",
  "message": "This LinkedIn profile has already been used for this application form."
}
```

## FormBricks Discovery Algorithm

Given authenticated Google user and `(kind, slug)`:

1. Load `application_forms` row where `kind = ? and slug = ?`.
2. Check `is_active`, `opens_at`, `closes_at`, `editable_until`.
3. Query FormBricks Management API responses for `formbricks_survey_id`.
4. Paginate until one of these conditions:
   - found email match,
   - found duplicate LinkedIn match when validating,
   - no more responses,
   - hit configured safety limit and return a controlled error/log.
5. Extract answer values from `response.data[email_question_id]` and `response.data[linkedin_question_id]`.
6. Normalize values.
7. Existing user application if normalized response email equals normalized Google email.
8. Duplicate LinkedIn if normalized LinkedIn equals proposed normalized LinkedIn and response email is not the same Google email.

Important: FormBricks response `data` keys are question IDs, not labels. Store question IDs in D1. Do not rely on label text like “Email Address” or “LinkedIn Profile URL” because admins may rename labels.

## Expiry and Archive Policy

- `opens_at`: form is visible/open after this date if set.
- `closes_at`: new applications blocked after this date if set.
- `editable_until`: existing applications can be edited until this date if set.
- `archive_after`: response is ignored/archived from active applicant pool after this date if set.
- Backend should enforce these policies even if the FormBricks public link still technically works.
- If FormBricks is directly public and bypassable, prefer returning links only through backend and avoid exposing long-lived generic public URLs where possible.

## Auth Plan

- User logs in with Google SSO with email/profile permission.
- Preferred if infrastructure allows: use Cloudflare Access with Google IdP and validate Cloudflare Access JWT in Worker.
- Alternative: frontend sends Google ID token to Worker; Worker validates issuer, audience, expiry, and Google JWKS signature. Confirm Rust/WASM-compatible JWT/JWK crates before implementation.
- Always compare normalized Google email to submitted Email Address answer for ownership.

## Cloudflare Worker Rust Plan

Current repo at onboarding time is minimal: `Cargo.toml`, empty root `main.rs`, no `wrangler.toml`.

Target layout:

```text
Cargo.toml
wrangler.toml
migrations/
  0001_application_forms.sql
src/
  lib.rs
  config.rs
  http/
    mod.rs
    routes.rs
    response.rs
    errors.rs
  auth/
    mod.rs
    google.rs
    user.rs
  application/
    mod.rs
    forms.rs
    service.rs
    policy.rs
    discovery.rs
  formbricks/
    mod.rs
    client.rs
    responses.rs
    types.rs
  storage/
    mod.rs
    d1.rs
  validation/
    mod.rs
    email.rs
    linkedin.rs
```

Recommended Rust dependencies:

- `worker` for Cloudflare Workers runtime.
- `serde`, `serde_json` for API and FormBricks payloads.
- `wasm-bindgen`, `console_error_panic_hook` for Worker/WASM ergonomics.
- A URL parser crate only if compatible with `wasm32-unknown-unknown`; otherwise implement constrained LinkedIn normalization manually.
- Auth/JWT crates must be checked for `wasm32-unknown-unknown` support before choosing.

Worker config/secrets:

- Vars:
  - `FORMBRICKS_BASE_URL` (self-hosted URL)
  - `ALLOWED_ORIGINS`
  - optional `GOOGLE_CLIENT_ID` or Access audience/team config
- Secrets:
  - `FORMBRICKS_API_KEY`
  - any Google auth secret/config if needed
- Bindings:
  - D1 database for `application_forms`

## Implementation Sequence For Another Agent

1. Convert repo to Cloudflare Worker Rust layout.
   - Move from root `main.rs` binary shape to `src/lib.rs` Worker entrypoint.
   - Add `wrangler.toml` and `worker-build` setup according to current Cloudflare workers-rs guidance.
2. Add config loading from Worker `Env`.
   - Read FormBricks base URL, API key secret, D1 binding, allowed origins.
3. Add common HTTP response/error helpers.
   - JSON success/error shape, CORS, method handling, path params.
4. Add D1 migration for `application_forms`.
   - Include indexes and unique `(kind, slug)`.
5. Implement storage repository for form registry.
   - `list_forms(kind?)`, `get_form(kind, slug)`.
6. Implement auth extraction.
   - Pick Cloudflare Access JWT or Google ID token flow based on deployment decision.
   - Return `AuthUser { sub, email, name, picture }`.
7. Implement validation utilities.
   - `normalize_email`.
   - `normalize_linkedin_profile_url` with strict `/in/{slug}` validation.
8. Implement FormBricks client.
   - Base URL configurable.
   - Always send `x-api-key` from Worker secret for Management API calls.
   - Implement paginated `list_responses(survey_id, limit, skip)` using v2 Management API.
   - Map FormBricks errors to backend errors without leaking API key.
9. Implement discovery service.
   - Given form + Google email, scan responses and find exact email match.
   - Given form + proposed LinkedIn URL, scan responses and find duplicate LinkedIn.
   - Keep pagination limits configurable/conservative.
10. Implement application policy checks.
   - active/open/closed/editable/archive logic.
11. Implement routes:
   - `GET /health`
   - `GET /api/forms`
   - `GET /api/forms/:kind`
   - `GET /api/forms/:kind/:slug`
   - `GET /api/applications/:kind/:slug`
   - `POST /api/applications/:kind/:slug/validate`
   - `POST /api/applications/:kind/:slug/link`
12. Add tests where practical.
   - email normalization,
   - LinkedIn normalization/rejection,
   - policy date checks,
   - discovery behavior with mocked FormBricks responses,
   - duplicate detection same LinkedIn/different email,
   - existing application same email.
13. Run completion checks:
   - `cargo fmt`
   - `cargo check`
   - `cargo test` if tests exist
   - `npx wrangler dev` smoke test once Worker config exists.

## Key Risks / Follow-ups

- Live FormBricks response scanning may be slow at high volume because uniqueness is by arbitrary question answer. Add webhook-backed `application_response_index` if needed.
- Direct public FormBricks links can bypass backend pre-validation. Prefer backend-generated links and webhook validation/indexing if hard enforcement is required.
- If FormBricks allows users to submit directly without backend mediation, duplicate prevention may be reactive rather than preventive. Decide whether reactive duplicate marking is acceptable.
- Need final decision on auth implementation: Cloudflare Access vs frontend Google ID token verification.
- Need an admin workflow to create/update `application_forms` records and keep `email_question_id` / `linkedin_question_id` accurate when forms are changed.