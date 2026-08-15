# Username-Based Profile Architecture & Decision Record

**Date**: 2026-08-16  
**Status**: Approved & Documented  
**Scope**: `jakarta-backend` & `jakarta-website`

---

## 1. Motivation: PII Prevention in Public Git Repositories

### Problem
Previously, the public people roster and event frontmatter in `jakarta-website` relied on contributors' email addresses (e.g. `organizers: [{ email: "user@example.com" }]`) to link event team members to their self-service profiles. Because `jakarta-website` is a public open-source git repository, storing real email addresses in MDX files directly exposed personal identifiable information (PII) to web scrapers, commit logs, and public history.

### Solution
Introduce public, unique `username` handles as the identifier for public profile lookups, content frontmatter, and roster configurations.
- **Email** remains strictly private: used only for Google OAuth authentication and internal D1 account linking. It is never exposed in public frontmatter, public git commits, or public lookup responses.
- **Username** acts as the public handle (e.g., `avei`, `johndoe`) used in event rosters, team cards, and profile URLs.

---

## 2. Username Format & Validation Rules

Usernames must conform to the following specifications:
- **Length**: 3 to 30 characters (`3..=30`).
- **Allowed Characters**: Lowercase alphanumeric (`a-z`, `0-9`), hyphens (`-`), and underscores (`_`).
- **Format Pattern**: `^[a-z0-9][a-z0-9_-]{1,28}[a-z0-9]$` (starts and ends with alphanumeric characters, lowercase, no spaces).
- **Case Sensitivity**: Normalized to lowercase on input/validation; case-insensitive uniqueness in the database.
- **Reserved / Collision Handling**: Unique across the system; attempting to claim an existing username returns `409 Conflict`.

---

## 3. Event Frontmatter Roster Syntax

In `jakarta-website` content files (e.g., `src/content/events/*.mdx`), event organizers and volunteers are specified using a compact colon-delimited string format or structured objects.

### Compact String Syntax:
```yaml
organizers:
  - avei:Muhammad Abdul Aziz:User Group Leader
  - sarah_dev:Sarah:Co-organizer
volunteers:
  - alex_k:Alex:Registration Lead
```
Format: `username:fallbackName:role`

### Fallback & Resolution Chain:
1. **Frontend Lookup**: Batches requested usernames via `POST /api/profiles/lookup`.
2. **Avatar / Photo**: Profile `picture_url` (from Google OAuth snapshot) → fallback placeholder.
3. **Display Name**: Custom `display_name` → OAuth `name` → `fallbackName` → `username`.
4. **Title / Role**: Custom profile `title` → frontmatter `role` → default label (e.g., "Community Member").
5. **Social Links**: Custom verified links from public profile (if `is_public = 1`).
6. **Unpublished / Unregistered Graceful Fallback**: If profile is private or not registered yet, displays `fallbackName` and `role` with default avatar styling without breaking layout.

---

## 4. D1 Database Schema (`profiles` table)

The `profiles` table in Cloudflare D1 incorporates a unique `username` column alongside profile customisation fields.

```sql
-- D1 Migration: profiles table schema
ALTER TABLE profiles ADD COLUMN username TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_profiles_username ON profiles(username);
```

### Full Schema Definition:
| Column | Type | Constraints | Description |
|---|---|---|---|
| `email` | `TEXT` | `PRIMARY KEY` | Internal user email from Google OAuth |
| `username` | `TEXT` | `UNIQUE` | Public handle (3-30 chars, lowercase `[a-z0-9_-]`) |
| `name` | `TEXT` | `NOT NULL` | Full name snapshot from Google OAuth |
| `picture_url` | `TEXT` | `NULL` | Profile photo URL from Google OAuth |
| `display_name` | `TEXT` | `NULL` | User-customized display name override |
| `title` | `TEXT` | `NULL` | User-customized role/headline |
| `links_json` | `TEXT` | `NULL` | JSON array of validated social/portfolio links |
| `is_public` | `INTEGER` | `DEFAULT 0` | Visibility flag (0 = private, 1 = public) |
| `created_at` | `DATETIME` | `DEFAULT CURRENT_TIMESTAMP` | Account creation timestamp |
| `updated_at` | `DATETIME` | `DEFAULT CURRENT_TIMESTAMP` | Internal row update timestamp |
| `profile_updated_at`| `DATETIME` | `NULL` | User profile edit timestamp |

---

---

## 6. Avatar Media Storage (Cloudflare R2)

To allow community members to upload custom profile photos rather than being restricted to their Google account photo, Cloudflare R2 object storage is integrated.

### 6.1 Infrastructure & Bindings
- **R2 Bucket Name**: `jakarta-bucket`
- **Worker Binding**: `AVATAR_BUCKET` (configured in `wrangler.toml` via `[[r2_buckets]]`)
- **Public CDN Domain**: `https://avatars.awscommunity.id`
- **Object Key Convention**: `avatars/{username_or_hash}_{timestamp}.{ext}` (served as `https://avatars.awscommunity.id/avatars/...`)

### 6.2 Validation Constraints & Guardrails
- **Max File Size**: 2MB (`2,097,152` bytes). Uploads exceeding this return `413 Payload Too Large` or `400 Bad Request`.
- **Supported Formats**: JPEG, PNG, WebP.
- **Magic-Byte Inspection**: Server strictly verifies initial file magic bytes before uploading to R2:
  - **JPEG**: `FF D8 FF`
  - **PNG**: `89 50 4E 47 0D 0A 1A 0A`
  - **WebP**: `52 49 46 46` (RIFF) ... `57 45 42 50` (WEBP)
  - Rejects disguised scripts, SVGs with embedded scripts, and arbitrary binary files with `400 Bad Request`.
- **HTTP Metadata**: Objects stored with `httpMetadata` headers: `Content-Type: image/{jpeg,png,webp}` and `Cache-Control: public, max-age=31536000, immutable`.

### 6.3 Revert-to-Google OAuth Avatar Lifecycle
1. **Initial State (Google OAuth)**: When a user authenticates, their Google profile photo (`https://lh3.googleusercontent.com/...`) is saved as the default `picture_url`.
2. **Custom Avatar Upload (`POST /api/profiles/me/avatar`)**:
   - The previous custom R2 avatar (if any) is purged from `AVATAR_BUCKET` to avoid orphaned objects.
   - The new image is uploaded to R2 and `picture_url` in D1 is updated to `https://avatars.awscommunity.id/...`.
3. **Revert Avatar (`DELETE /api/profiles/me/avatar`)**:
   - The custom object is deleted from `AVATAR_BUCKET`.
   - The `picture_url` in D1 is reverted to the snapshot Google OAuth photo URL (or `NULL` if no Google photo exists).

---

## 7. API Contracts (Complete)

### 7.1 `GET /api/profiles/me`
- **Auth**: Required (`Authorization: Bearer <google_id_token>` or `X-Debug-User-Email`).
- **Purpose**: Retrieve current user's profile and self-service settings.
- **Response**:
```json
{
  "email": "user@example.com",
  "username": "johndoe",
  "name": "John Doe",
  "picture": "https://avatars.awscommunity.id/avatars/johndoe_1713000000.webp",
  "displayName": "Johnny D",
  "title": "Cloud Architect",
  "links": [
    { "platform": "github", "url": "https://github.com/johndoe" },
    { "platform": "linkedin", "url": "https://linkedin.com/in/johndoe" }
  ],
  "isPublic": true,
  "profileUpdatedAt": "2026-08-16T00:00:00Z"
}
```

### 7.2 `PUT /api/profiles/me`
- **Auth**: Required (`Authorization: Bearer <google_id_token>` or `X-Debug-User-Email`).
- **Purpose**: Update self-service profile details, including choosing or changing username.
- **Request Body**:
```json
{
  "username": "johndoe",
  "displayName": "Johnny D",
  "title": "Cloud Architect",
  "links": [
    { "platform": "github", "url": "https://github.com/johndoe" },
    { "platform": "linkedin", "url": "https://linkedin.com/in/johndoe" }
  ],
  "isPublic": true
}
```
- **Responses**:
  - `200 OK`: Profile successfully updated.
  - `400 Bad Request`: Invalid username format (e.g. invalid length, invalid characters) or invalid links.
  - `409 Conflict`: Username is already claimed by another user.

### 7.3 `POST /api/profiles/me/avatar`
- **Auth**: Required (`Authorization: Bearer <google_id_token>` or `X-Debug-User-Email`).
- **Content-Type**: `multipart/form-data` with `file` field or raw binary `image/jpeg`, `image/png`, `image/webp`.
- **Validation**:
  - Max 2MB file size.
  - Magic-byte validation (`JPEG`, `PNG`, `WebP`).
- **Response**: `200 OK` with updated `MyProfile` payload containing the newly generated R2 CDN URL.

### 7.4 `DELETE /api/profiles/me/avatar`
- **Auth**: Required (`Authorization: Bearer <google_id_token>` or `X-Debug-User-Email`).
- **Purpose**: Delete custom uploaded avatar from R2 and restore the Google OAuth profile photo.
- **Response**: `200 OK` with updated `MyProfile` payload containing the restored Google OAuth photo URL (or `null`).

### 7.5 `POST /api/profiles/lookup`
- **Auth**: Public (No authentication required).
- **Purpose**: Bulk resolve public profile cards for event rosters, speaker highlights, and team displays.
- **Request Body**:
```json
{
  "usernames": ["avei", "johndoe", "sarah_dev"]
}
```
- **Response**:
```json
{
  "profiles": [
    {
      "normalized_email": "avei@awscommunity.id",
      "username": "avei",
      "display_name": "Avei",
      "title": "AWS Community Leader",
      "picture": "https://avatars.awscommunity.id/avatars/avei_1713000000.webp",
      "links": [
        { "platform": "github", "url": "https://github.com/avei" }
      ],
      "profile_updated_at": "2026-08-16T00:00:00Z"
    }
  ]
}
```
- **Privacy & Filtering Rules**:
  - Only returns records where `is_public = 1`. Private profiles or nonexistent usernames are omitted.
  - The `email` field is **never** included in public frontmatter or public API lookups.
