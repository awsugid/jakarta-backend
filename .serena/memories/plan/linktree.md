# Customizable Link Page Plan

## Goal

Add Linktree-like public page to `jakarta-website`, configurable from existing `/admin`, persisted in Cloudflare D1 through `jakarta-backend`.

## Verified architecture

- `jakarta-website`: Astro static output, React 19 islands, Tailwind CSS v4.
- `jakarta-backend`: Rust Cloudflare Worker using `worker 0.8`.
- Database: Cloudflare D1 binding `DB`.
- Existing admin auth: Google ID token plus backend `ADMIN_EMAILS` allowlist.
- Every `/api/admin/*` handler must call `require_admin`; router has no middleware API.
- Static Astro page cannot fetch fresh DB content during request. Public link page needs client-side runtime fetch unless project adopts SSR adapter.
- Existing `/admin` dashboard and frontend API/auth utilities should be reused.

## Proposed MVP decisions

These remain assumptions until user confirms:

- One community link page, not multi-tenant profiles.
- Public route: `/links`.
- Existing `/admin` gains Links section.
- Editable profile fields: title, bio, avatar URL, background variant, button variant.
- Editable link fields: label, URL, icon name, enabled state, display order.
- Use avatar URL; no file upload.
- Fixed style variants; no arbitrary CSS, HTML, or Tailwind classes from database.
- Reordering uses accessible up/down buttons; no drag-and-drop dependency.
- No analytics, link scheduling, custom domains, or multi-page support in MVP.

## Phase 1 — D1 schema

Create `jakarta-backend/migrations/0004_link_page.sql`.

### `link_page`

Singleton row:

- `id INTEGER PRIMARY KEY CHECK (id = 1)`
- `title TEXT NOT NULL`
- `bio TEXT`
- `avatar_url TEXT`
- `background TEXT NOT NULL`
- `button_style TEXT NOT NULL`
- `updated_at TEXT NOT NULL DEFAULT (datetime('now'))`

### `link_items`

- `id TEXT PRIMARY KEY`
- `label TEXT NOT NULL`
- `url TEXT NOT NULL`
- `icon TEXT`
- `is_enabled INTEGER NOT NULL CHECK (is_enabled IN (0, 1))`
- `display_order INTEGER NOT NULL`
- `created_at TEXT NOT NULL DEFAULT (datetime('now'))`
- `updated_at TEXT NOT NULL DEFAULT (datetime('now'))`

Seed singleton page with community defaults. Do not require unique `display_order`; reorder operation updates ordered rows.

## Phase 2 — Backend domain and repository

Add minimum module files:

- `jakarta-backend/src/links/mod.rs`
- `jakarta-backend/src/links/types.rs`
- `jakarta-backend/src/links/repository.rs`

Operations:

- `get_public_page()` — profile plus enabled links ordered by `display_order, created_at`.
- `get_admin_page()` — profile plus all links.
- `update_page()`.
- `create_link()`.
- `update_link()`.
- `delete_link()`.
- `reorder_links()`.

Use bound D1 parameters. Keep transactions/batches where reorder consistency requires them.

## Phase 3 — Backend HTTP API

Add `jakarta-backend/src/http/links.rs`. Register routes in `jakarta-backend/src/http/routes.rs`.

### Public

- `GET /api/links`

Returns page settings and enabled links. Add `Cache-Control: public, max-age=60` only if one-minute update delay is accepted.

### Admin

- `GET /api/admin/links`
- `PUT /api/admin/links/page`
- `POST /api/admin/links/items`
- `PUT /api/admin/links/items/:linkId`
- `DELETE /api/admin/links/items/:linkId`
- `PUT /api/admin/links/order`

Every admin handler calls `require_admin` before D1 access.

### Boundary validation

- Title: 1–100 characters.
- Bio: maximum 300 characters.
- Label: 1–80 characters.
- URL: maximum 2048 characters; allow only `http:` and `https:`.
- Icon: fixed allowlist matching frontend icons.
- Background and button styles: fixed allowlists.
- Reorder IDs: non-empty, unique, bounded item count, all IDs must exist.
- Unknown item: 404.
- Invalid input: 400.
- Database errors: existing `AppError` response pattern.

## Phase 4 — Website API contracts

Update existing files:

- `jakarta-website/src/lib/types.ts`
- `jakarta-website/src/lib/api.ts`

Add shared types and functions:

- `fetchPublicLinks()`.
- `fetchAdminLinks()`.
- `updateLinkPage()`.
- `createLink()`.
- `updateLink()`.
- `deleteLink()`.
- `reorderLinks()`.

Reuse existing backend URL and Google token attachment. Add no dependency.

## Phase 5 — Public page

Add:

- `jakarta-website/src/pages/links.astro`
- `jakarta-website/src/components/links/LinkPage.tsx`

`links.astro` is static shell. Render `LinkPage` as a React island using `client:visible`; component fetches `GET /api/links` in browser.

Required states:

- Loading skeleton.
- Loaded profile and links.
- Empty link list.
- Retryable API error.

Accessibility and security:

- Real `<a>` elements.
- Visible keyboard focus.
- Meaningful avatar alt text.
- External links use `rel="noopener noreferrer"`.
- Mobile-first layout at 360–414 px.
- Render only fixed background/button variants.
- Never inject database HTML or database-provided class names.

## Phase 6 — Admin customization UI

Keep `jakarta-website/src/pages/admin/index.astro` and existing `AdminGuard`. Add smallest useful navigation and editor components:

- `jakarta-website/src/components/admin/AdminNavigation.tsx`
- `jakarta-website/src/components/admin/LinkManager.tsx`
- `jakarta-website/src/components/admin/LinkEditor.tsx`

Features:

- Edit profile title, bio, avatar URL, background, button style.
- Add/edit links.
- Enable/disable links.
- Delete with confirmation.
- Reorder with up/down buttons.
- Explicit profile save.
- Preserve unsaved values when API returns validation errors.
- Wait for successful writes before reflecting persisted state; optimistic updates deferred.

## Parallel execution plan

Use disjoint write scopes to avoid merge conflicts.

### Wave 1 — independent work

#### Agent A: database and backend repository

Write scope:

- `jakarta-backend/migrations/0004_link_page.sql`
- `jakarta-backend/src/links/**`
- module declaration needed in `jakarta-backend/src/lib.rs` only if assigned exclusively to Agent A.

Output contract: repository methods and Rust request/response types agreed before Wave 2.

#### Agent B: public website UI

Write scope:

- `jakarta-website/src/pages/links.astro`
- `jakarta-website/src/components/links/**`

Use temporary local TypeScript interface matching agreed API response. Do not edit shared `src/lib/types.ts` or `src/lib/api.ts` during this wave.

#### Agent C: admin UI design slice

Write scope:

- new `jakarta-website/src/components/admin/LinkManager.tsx`
- new `jakarta-website/src/components/admin/LinkEditor.tsx`
- new `jakarta-website/src/components/admin/AdminNavigation.tsx`

Accept API functions through imports agreed in advance. Do not edit existing dashboard or shared API files during this wave.

### Wave 2 — after repository contract exists

#### Agent D: backend handlers and tests

Write scope:

- `jakarta-backend/src/http/links.rs`
- route additions in `jakarta-backend/src/http/routes.rs`
- HTTP-focused tests where supported.

Depends on Agent A repository/types.

#### Agent E: website shared API integration

Write scope:

- `jakarta-website/src/lib/types.ts`
- `jakarta-website/src/lib/api.ts`

Depends on final backend JSON contract. Reconcile temporary local public UI types by exporting canonical types.

### Wave 3 — integration

Single integrator only, avoiding conflicts:

- Wire admin navigation into `jakarta-website/src/components/admin/AdminDashboard.tsx`.
- Replace temporary types/imports in public UI.
- Review CORS, auth token, loading/error behavior, and route names.
- Apply formatting.

### Wave 4 — parallel validation

#### Backend validation agent

Run:

- `cargo fmt --check`
- `cargo test`
- `cargo check`
- local D1 migration application.
- API checks for public filtering, 401, 403, invalid URL, CRUD, reorder persistence.

#### Website validation agent

Run from `jakarta-website` after `nvm use 22`:

- `bunx astro check`
- `bun run build`
- browser checks at mobile and desktop widths.
- keyboard navigation and admin state checks.

#### Security/review agent

Read-only review:

- Confirm all admin handlers call `require_admin`.
- Confirm SQL uses bindings.
- Confirm `javascript:` and other URL schemes are rejected.
- Confirm database values cannot become raw HTML or arbitrary CSS classes.
- Confirm no secrets enter diff.

Integrator fixes only findings caused by feature.

## Dependency graph

1. Agree MVP decisions and JSON contract.
2. Wave 1 runs in parallel.
3. Backend HTTP and website API integration run in parallel after repository contract stabilizes.
4. Single integration pass.
5. Validation runs in parallel.
6. Single final review and fixes.

## Validation acceptance criteria

- Public page shows enabled links in stored order.
- Disabled links never appear publicly.
- Admin can load, create, update, toggle, delete, and reorder links.
- Unauthenticated admin requests return 401.
- Authenticated non-admin requests return 403.
- Invalid URL schemes return 400.
- Refresh preserves all changes.
- Public page works at 360 px and with keyboard navigation.
- `cargo fmt --check`, `cargo test`, `cargo check`, `bunx astro check`, and `bun run build` pass, excluding documented pre-existing failures.

## Deliberate MVP ceiling

Skipped: file upload, arbitrary themes, click analytics, scheduling, custom domains, drag-and-drop, and multi-tenant profiles. Add only when one community page proves insufficient.

## Confirmed questions for user

1. Scope: one AWS User Group Jakarta link page, or multiple profiles/pages?
2. Public URL: `/links`, root `/`, or custom slug such as `/connect`?
3. Admin placement: new Links tab inside existing `/admin`, or separate `/admin/links` route?
4. Avatar: URL-only for MVP, or direct image upload required now?
5. Public freshness: accept up to 60 seconds cache delay, or require immediate updates?
6. Initial links/content: seed known community links, or start empty?
