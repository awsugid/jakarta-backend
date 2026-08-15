# Backend API Endpoints — Frontend Integration Plan

**Base URL**: The backend is deployed as a Cloudflare Worker at `jakarta-backend` subdomain (to be configured — currently at Cloudflare).

## API Endpoints

### Public (No Auth Required)

| Method | Path | Purpose | Response |
|--------|------|---------|----------|
| `GET` | `/health` | Health check | `"OK"` |
| `GET` | `/api/forms?kind=volunteer\|speaker` | List all active forms, optionally filtered by kind | `[{ kind, slug, title, description, survey_id, is_active, opens_at, closes_at, editable_until }]` |
| `GET` | `/api/forms/:kind` | List active forms for a kind | Same as above, filtered |
| `GET` | `/api/forms/:kind/:slug` | Get single form with policy status | `{ form: FormInfo, status: "open" \| "closed" \| "not_yet_open" \| "archived" }` |

| `POST` | `/api/profiles/lookup` | Public bulk lookup of public profiles by username | `{ usernames: string[] }` | `{ profiles: { [username]: PublicProfile } }` (only `is_public=1`) |

### Auth-Required (Google Auth — Bearer token or X-Debug-User-Email for dev)

| Method | Path | Purpose | Request Body | Response |
|--------|------|---------|-------------|----------|
| `GET` | `/api/applications/:kind/:slug` | Discover user's existing application | — | `{ exists, response_id?, finished?, submitted_email?, linkedin_url?, editable }` |
| `POST` | `/api/applications/:kind/:slug/validate` | Validate before submit / check duplicate LinkedIn | `{ linkedin_url: "..." }` | `{ ok, code?, message? }` e.g. `{ ok: false, code: "duplicate_linkedin", message: "..." }` |
| `POST` | `/api/applications/:kind/:slug/link` | Get FormBricks form link (only if open/editable) | — | `{ url: "...", editable: bool }` |
| `GET` | `/api/profiles/me` | Get self-service profile | — | Profile JSON with `username`, `display_name`, `title`, `links`, etc. |
| `PUT` | `/api/profiles/me` | Update self-service profile | `{ username, display_name, title, links, is_public }` | `200 OK` or `400`/`409 Conflict` |

## Auth Headers

- **Production**: `Authorization: Bearer <google-id-token>` (JWT validation not yet implemented; currently returns 401 with a message)
- **Dev**: `X-Debug-User-Email: user@example.com` (returns a debug AuthUser)

## Kinds

- `"volunteer"` — Volunteer application forms (divisions: foh, logistics, registration, design, documentation, event, etc.)
- `"speaker"` — Speaker application forms (single form, no division cards)

## Frontend Integration Flow

### For Speakers Page (`/speakers`):

1. On page load, call `GET /api/forms/speaker` to get the speaker form metadata (title, description, policy status).
2. If form has `status: "open"`, show "Apply as Speaker" CTA button.
3. If user clicks "Apply":
   a. Check if user is authenticated (Google Sign-In).
   b. Call `GET /api/applications/speaker/:slug` to check if they already have an application.
   c. If `exists: true`, show their existing application status (finished, editable).
   d. If `exists: false`, call `POST /api/applications/speaker/:slug/link` to get the FormBricks URL.
   e. Redirect user to the FormBricks URL or embed it.

### For Volunteer Page (`/volunteer`):

1. On page load, call `GET /api/forms/volunteer` to get all volunteer division cards.
2. Map each division card (foh, logistics, registration, etc.) to a volunteer form.
3. For each card, show its status (open/closed/not_yet_open) and an "Apply" button if open.
4. Same flow as speakers for applying: auth → discovery → link.

### Google Sign-In

- Frontend must implement Google Sign-In (OAuth 2.0) to get an ID token.
- Send the ID token as `Authorization: Bearer <id_token>` to the backend.
- Backend will validate the token (JWT validation TBD — currently only dev header works).
- Once backend JWT validation is implemented, the flow is: Sign in → get token → send with every auth request.

## Error Format

```json
{
  "error": {
    "code": 401,
    "message": "Authentication required"
  }
}
```

## CORS

- `Access-Control-Allow-Origin` set to `ALLOWED_ORIGINS` env var (currently `https://jakarta.awscommunity.id`)
- `Access-Control-Allow-Methods`: `GET, POST, PUT, DELETE, OPTIONS`
- `Access-Control-Allow-Headers`: `Content-Type, Authorization`
- Preflight handled at `OPTIONS /api/*rest`
