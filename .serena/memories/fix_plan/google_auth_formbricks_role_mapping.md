# Fix Plan — Google Auth, Formbricks Mapping, Volunteer Multi-Form, Speaker Single-Form

## Goal

Backend must support:

1. Multiple volunteer application forms, one Formbricks survey per volunteer card/division.
2. Single speaker application form, one Formbricks survey for all speaker submissions.
3. Google SSO authentication with frontend Google ID token.
4. Formbricks-backed discovery and link generation through D1 `application_forms` registry.

## Current Root Causes

1. `src/auth/google.rs` rejects real `Authorization: Bearer <google_id_token>` because JWT validation is not implemented.
2. Frontend sends only `Authorization: Bearer ...`; backend dev fallback `X-Debug-User-Email` is not used by production flow.
3. `ALLOWED_ORIGINS` in `wrangler.toml` only contains `https://jakarta.awscommunity.id`, so local frontend may fail CORS.
4. D1 `application_forms` is the required role/form mapping, but it needs seeded rows.
5. Backend already supports `(kind, slug)` generically, so no schema change is needed for single speaker form. Use one row: `kind='speaker'`, `slug='speaker'`.

## Backend Implementation Plan

### 1. Implement Google ID token validation

File: `src/auth/google.rs`

Replace current Bearer-token stub with real Google ID token verification.

Required checks:

- JWT has 3 parts.
- Decode header, get `kid`, `alg` must be `RS256`.
- Fetch Google JWKS from `https://www.googleapis.com/oauth2/v3/certs`.
- Verify RS256 signature using matching JWK.
- Validate payload:
  - `iss` is `https://accounts.google.com` or `accounts.google.com`.
  - `aud` equals backend env var `GOOGLE_CLIENT_ID`.
  - `exp` is in the future.
  - `email_verified` is true.
  - `email` exists.
- Return `AuthUser { sub, email, name, picture }`.

Add config:

- `GOOGLE_CLIENT_ID` as Worker var.

Update `src/config.rs`:

```rust
pub google_client_id: String,
```

Update `wrangler.toml`:

```toml
GOOGLE_CLIENT_ID = "your-google-client-id.apps.googleusercontent.com"
```

Keep `X-Debug-User-Email` only for local/dev, but gate it behind an explicit env var if possible:

```toml
ENABLE_DEBUG_AUTH = "false"
```

### 2. Fix CORS config

File: `wrangler.toml`

Current:

```toml
ALLOWED_ORIGINS = "https://jakarta.awscommunity.id"
```

Target should include production and local dev. If response helper supports exact string only, update response helper to parse comma-separated origins and echo back matching request origin.

Target var:

```toml
ALLOWED_ORIGINS = "https://jakarta.awscommunity.id,http://localhost:4321"
```

Files to inspect/update:

- `src/http/response.rs`
- `src/config.rs`

### 3. Keep D1 mapping model

No schema change required.

D1 table `application_forms` remains source of truth for:

- `kind`: `volunteer` or `speaker`
- `slug`: frontend route/card key
- `formbricks_survey_id`: Formbricks survey ID
- `formbricks_public_url`: public survey URL returned by `/link`
- `email_question_id`: exact Formbricks question ID for submitted email
- `linkedin_question_id`: exact Formbricks question ID for LinkedIn profile
- `is_active`, `opens_at`, `closes_at`, `editable_until`, `archive_after`

### 4. Volunteer: one form per card

Seed one row per current frontend card slug:

- `registration`
- `foh`
- `logistics`
- `design`
- `documentation`
- `event`
- `runner`
- `social-media`
- `liaison-officer`
- `sponsorship`
- `moderator-mc`
- `website`

Each row has `kind='volunteer'` and its own `formbricks_survey_id` / `formbricks_public_url`.

### 5. Speaker: single shared form

Use only one backend row:

- `kind='speaker'`
- `slug='speaker'`
- `title='Speaker Application'`

All speaker CTAs should point to `/api/applications/speaker/speaker` and `/api/applications/speaker/speaker/link`.

Do not create one speaker D1 row per talk format.

### 6. Link endpoint behavior

File: `src/http/routes.rs`

`POST /api/applications/:kind/:slug/link` currently returns generic `formbricks_public_url`.

Acceptable phase-1 behavior:

```json
{ "url": "https://forms.awscommunity.id/s/<survey-id>", "editable": true }
```

Optional improvement: append known user metadata as query params if Formbricks form supports hidden fields:

```text
?email=<google_email>&name=<google_name>
```

Only do this if confirmed safe and supported by the configured Formbricks survey.

### 7. Discovery and duplicate validation

Existing files:

- `src/application/discovery.rs`
- `src/formbricks/client.rs`

Keep current behavior:

- Existing application: scan Formbricks responses for matching `email_question_id` answer.
- Duplicate LinkedIn: scan same survey for matching `linkedin_question_id` answer with a different email.

Known limitation:

- Because the frontend redirects to public Formbricks, `validate` is not enforced before final submission unless frontend collects LinkedIn first or Formbricks webhook/indexing is added later.

### 8. Backend validation checklist

Run from `jakarta-backend`:

```bash
cargo fmt
cargo check
cargo test
```

Then smoke-test:

```bash
npx wrangler dev
curl http://localhost:8787/health
curl http://localhost:8787/api/forms?kind=volunteer
curl http://localhost:8787/api/forms?kind=speaker
```

Authenticated smoke-test after Google JWT validation:

```bash
curl -H "Authorization: Bearer <google_id_token>" http://localhost:8787/api/applications/speaker/speaker
```
