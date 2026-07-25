# Linktree feature — canonical contract & decisions

Authoritative shared spec. All sub-agents read this before coding.

## Decisions (confirmed defaults)

1. Single community link page (no multi-tenant).
2. Public route: `/links` (Astro static shell + React island `client:visible`).
3. Admin: new section inside existing `/admin` dashboard. Same `AdminGuard`. Use tab toggle in `AdminDashboard.tsx`.
4. Avatar: URL-only. No upload.
5. Public freshness: immediate updates. No cache layer.
6. Seed: singleton page seeded on migrate with community defaults; `link_items` starts empty.

## JSON contract (camelCase via serde rename_all = "camelCase")

### `GET /api/links` (public, no auth)
### `GET /api/admin/links` (admin, require_admin)

```
Response 200:
{
  "page": LinkPage,
  "items": LinkItem[]
}
```
Public variant filters `items` to `is_enabled = 1` ordered by `display_order, created_at`.
Admin variant returns ALL items ordered same way.

### `LinkPage`
```
{
  "title": string,           // 1..=100 chars
  "bio": string | null,      // null or 0..=300 chars
  "avatarUrl": string | null,
  "background": "dark" | "gradient" | "mesh",
  "buttonStyle": "solid" | "outline" | "soft",
  "updatedAt": string        // ISO 8601 from D1 datetime('now')
}
```

### `LinkItem`
```
{
  "id": string,              // server-generated, UUID v4 from js crypto.randomUUID()
  "label": string,           // 1..=80 chars
  "url": string,             // http: or https: only, <= 2048 chars
  "icon": string | null,     // must be in ICON_ALLOWLIST
  "isEnabled": boolean,      // stored as INTEGER 0/1 in D1
  "displayOrder": number     // integer; reassigned on reorder
}
```

### `PUT /api/admin/links/page`
Body: `{ title, bio, avatarUrl, background, buttonStyle }` (all required except `bio`/`avatarUrl` may be null)
Returns: updated `LinkPage`.

### `POST /api/admin/links/items`
Body: `{ label, url, icon }` (icon may be null)
Returns: created `LinkItem` (server assigns id + displayOrder = max+1).

### `PUT /api/admin/links/items/:id`
Body: `{ label, url, icon, isEnabled }`
Returns: updated `LinkItem`.
404 if id missing.

### `DELETE /api/admin/links/items/:id`
Returns: `204 No Content`.
404 if id missing.

### `PUT /api/admin/links/order`
Body: `{ ids: string[] }` — full ordered id list, non-empty, unique, all must exist, count <= 200.
Server reassigns `display_order = index` for each id in received order.
Returns: full updated `items` array.

## Allowlists

```
ICON_ALLOWLIST = [
  "link", "github", "linkedin", "twitter", "instagram", "youtube",
  "globe", "mail", "calendar", "map-pin", "users", "external-link"
]
BACKGROUND_ALLOWLIST = ["dark", "gradient", "mesh"]
BUTTON_STYLE_ALLOWLIST = ["solid", "outline", "soft"]
```

## Validation rules (return 400 on violation)

- `title`: trim, 1..=100 chars
- `bio`: null OR 0..=300 chars
- `avatarUrl`: null OR http/https URL <= 2048 chars
- `label`: trim, 1..=80 chars
- `url`: http or https scheme, <= 2048 chars, reject `javascript:`, `data:`, etc.
- `icon`: null OR in ICON_ALLOWLIST
- `background`: in BACKGROUND_ALLOWLIST
- `buttonStyle`: in BUTTON_STYLE_ALLOWLIST
- reorder ids: non-empty, all unique, all exist, count <= 200

## Status codes (existing AppError pattern)

- 400 → `AppError::BadRequest(msg)`
- 401 → returned by `require_admin` via auth/google when no token
- 403 → `AppError::Forbidden` when token valid but not admin, or item not found in non-public mutation paths (use 404 instead for missing items)
- 404 → `AppError::NotFound`
- 500 → `AppError::Internal`

## D1 schema (migration `0004_link_page.sql`)

```sql
CREATE TABLE IF NOT EXISTS link_page (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  title TEXT NOT NULL,
  bio TEXT,
  avatar_url TEXT,
  background TEXT NOT NULL,
  button_style TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO link_page (id, title, bio, avatar_url, background, button_style)
VALUES (1, 'AWS User Group Jakarta', 'Cloud community based in Jakarta. Join our meetups and events.',
        'https://avatars.githubusercontent.com/u/0', 'dark', 'solid')
ON CONFLICT(id) DO NOTHING;

CREATE TABLE IF NOT EXISTS link_items (
  id TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  url TEXT NOT NULL,
  icon TEXT,
  is_enabled INTEGER NOT NULL CHECK (is_enabled IN (0, 1)),
  display_order INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_link_items_order
  ON link_items(is_enabled, display_order, created_at);
```

## Backend module layout (Agent A)

- `jakarta-backend/migrations/0004_link_page.sql`
- `jakarta-backend/src/links/mod.rs` — re-exports
- `jakarta-backend/src/links/types.rs` — serde structs (camelCase)
- `jakarta-backend/src/links/repository.rs` — `LinkRepository` with `D1Database`
- Add `mod links;` to `jakarta-backend/src/lib.rs`

Mirror `jakarta-backend/src/storage/d1.rs` repository style. Use `wasm_bindgen::JsValue::from_str(...)` for binds and `js_sys`/`crypto.randomUUID()` via `web_sys` if available, else a small JS bridge:
```rust
fn gen_id() -> String {
    use wasm_bindgen::JsCast;
    let crypto = js_sys::Reflect::get(&web_sys::window().unwrap(), &"crypto".into()).unwrap();
    let v = js_sys::Reflect::get(&crypto, &"randomUUID".into()).unwrap();
    let f = js_sys::Function::from(v);
    f.call0(&crypto).unwrap().as_string().unwrap()
}
```
(Agent A: verify `web_sys` availability; Cargo.toml has no `web-sys` directly. Use `js_sys` + worker re-export, OR simpler: call worker-provided global. If blocked, request `getrandom` or `uuid` crate addition is NOT allowed — instead construct id from `js_sys::Date::now()` ms + `Math.random()` hex. Show working code.)

Repository methods:
- `get_page() -> Result<LinkPage>`
- `get_items(enabled_only: bool) -> Result<Vec<LinkItem>>`
- `update_page(input: &PageUpdate) -> Result<LinkPage>`
- `create_link(input: &LinkCreate) -> Result<LinkItem>` (assigns display_order = max+1)
- `update_link(id: &str, input: &LinkUpdate) -> Result<LinkItem>` (NotFound if missing)
- `delete_link(id: &str) -> Result<()>` (NotFound if missing)
- `reorder(ids: &[String]) -> Result<Vec<LinkItem>>` (validate all exist + unique; transactional via `db.batch()`)

## Backend HTTP layout (Agent D — Wave 2)

- `jakarta-backend/src/http/links.rs` — handlers
- Register routes in `jakarta-backend/src/http/routes.rs` (add 7 routes BEFORE the `.options("/api/*rest")` line; place after `/api/admin/formbricks/responses/:responseId` block)
- Add `pub mod links;` to `jakarta-backend/src/http/mod.rs`

Mirror `jakarta-backend/src/http/admin.rs` for handler pattern:
- Public handlers use `json_success(...)` + `with_cors(resp, &config.allowed_origins)`
- Admin handlers use `require_admin(&req, &config).await?` then `json_success_cors(&body, &config.allowed_origins, origin.as_deref())`
- DELETE returns 204: `Response::empty()?.with_status(204)` plus CORS headers

## Website layout

Wave 1:
- `jakarta-website/src/pages/links.astro` — static shell wrapping `<LinkPage client:visible />`
- `jakarta-website/src/components/links/LinkPage.tsx` — React island; defines LOCAL types matching contract above (until Agent E exports canonical ones)
- `jakarta-website/src/components/admin/AdminNavigation.tsx` — tab toggle props: `{ active: "responses" | "links", onChange }`
- `jakarta-website/src/components/admin/LinkManager.tsx` — orchestrator (loads, lists, edit dialog)
- `jakarta-website/src/components/admin/LinkEditor.tsx` — form for editing a single link or page settings

Wave 2 (Agent E):
- Add types + api funcs to `src/lib/types.ts` and `src/lib/api.ts`
- Replace local types in `links/LinkPage.tsx` with `@/lib/types` imports

Wave 3 (integrator):
- Update `AdminDashboard.tsx` to use `AdminNavigation` + render `LinkManager` when tab="links"

## Style variants (frontend only)

Background classes map:
- `dark`     → `bg-background`
- `gradient` → `bg-gradient-to-b from-background via-primary/10 to-background`
- `mesh`     → `bg-[radial-gradient(circle_at_30%_20%,theme(colors.primary/0.15),transparent),radial-gradient(circle_at_70%_80%,theme(colors.primary/0.10),transparent)] bg-background`

Button variant classes (use cn() with existing `Button` variants where possible):
- `solid`   → `bg-primary text-primary-foreground hover:bg-primary/90`
- `outline` → `border border-border bg-card hover:bg-muted`
- `soft`    → `bg-primary/10 text-primary hover:bg-primary/20`

Mobile-first container: `min-h-screen flex flex-col items-center px-4 py-10 max-w-md mx-auto`.

Accessibility:
- Real `<a href target="_blank" rel="noopener noreferrer">`
- Visible focus ring
- Avatar `<img alt={page.title}>`
- Keyboard navigation works (links are real anchors)

Security:
- NEVER inject DB HTML — render via React JSX (auto-escapes)
- NEVER use DB values as class names directly except via fixed allowlist lookups

## Validation acceptance

- `cargo fmt --check` and `cargo check` pass on backend
- `bunx astro check` and `bun run build` pass on website
- All admin handlers call `require_admin`
- All SQL uses bound parameters
- `javascript:` URLs rejected with 400
- DB values never reach HTML as raw strings
