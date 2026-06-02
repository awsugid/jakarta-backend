# Local Run Instructions — jakarta-backend

The backend is a Rust Cloudflare Worker. It can run locally with Wrangler.

## Prerequisites

- Node.js 22 is required for Wrangler 4.
- Rust toolchain must be installed.
- `worker-build` is installed automatically by the custom Wrangler build command.
- Local D1 binding is provided by Wrangler from `wrangler.toml`.

## D1 migrations

`wrangler dev` does not automatically run D1 migrations. If local requests fail with:

```text
D1_ERROR: no such table: application_forms
```

apply local migrations first:

```bash
. /home/avei/.nvm/nvm.sh
nvm use 22
npx wrangler d1 migrations apply jakarta-backend --local
```

Production/remote migrations also do not run automatically in the current CI. Current `.github/workflows/deploy.yml` deploys the Worker but does not call `wrangler d1 migrations apply`.

Apply production migrations manually when needed:

```bash
npx wrangler d1 migrations apply jakarta-backend --remote
```

Recommended CI addition before Worker deploy:

```yaml
- name: Apply D1 migrations
  run: npx wrangler d1 migrations apply jakarta-backend --remote
  env:
    CLOUDFLARE_API_TOKEN: ${{ secrets.CLOUDFLARE_API_TOKEN }}
    CLOUDFLARE_ACCOUNT_ID: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
```

## Run locally

From `jakarta-backend`:

```bash
. /home/avei/.nvm/nvm.sh
nvm use 22
npx wrangler d1 migrations apply jakarta-backend --local
npx wrangler dev --local --port 8787
```

Expected ready output:

```text
[custom build] Finished `release` profile [optimized]
[custom build] Your wasm pkg is ready to publish
⎔ Starting local server...
[wrangler:info] Ready on http://localhost:8787
```

`wrangler dev` is long-running. If run through an agent terminal with a timeout, a timeout is acceptable after the `Ready on http://localhost:8787` line appears.

## Smoke tests

In another shell while Wrangler is running:

```bash
curl http://localhost:8787/health
curl http://localhost:8787/api/forms?kind=volunteer
curl http://localhost:8787/api/forms?kind=speaker
```

## Authenticated local testing

Authenticated endpoints require one of these:

1. A real Google ID token whose audience matches `GOOGLE_CLIENT_ID`.
2. Debug auth enabled in `wrangler.toml`:

```toml
ENABLE_DEBUG_AUTH = "true"
```

Then call with:

```bash
curl \
  -H "X-Debug-User-Email: dev@example.com" \
  http://localhost:8787/api/applications/speaker/speaker
```

Without valid Google auth or debug auth, these endpoints return `401`:

```text
GET  /api/applications/:kind/:slug
POST /api/applications/:kind/:slug/validate
POST /api/applications/:kind/:slug/link
```

## Current local vars from `wrangler.toml`

```toml
FORMBRICKS_BASE_URL = "https://forms.awscommunity.id"
ALLOWED_ORIGINS = "*"
GOOGLE_CLIENT_ID = ""
ENABLE_DEBUG_AUTH = "false"
```

Set a real `GOOGLE_CLIENT_ID` before production deploy. `FORMBRICKS_API_KEY` must be configured as a Wrangler secret:

```bash
npx wrangler secret put FORMBRICKS_API_KEY
```
