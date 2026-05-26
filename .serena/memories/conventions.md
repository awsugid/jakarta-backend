# Conventions

- Current codebase has no established Rust module conventions yet; prefer introducing modules deliberately rather than packing all Worker logic into root `main.rs`.
- Keep backend role as proxy/orchestration layer over FormBricks, not as the source of truth for submitted form data unless explicitly added later.
- Avoid committing secrets; Cloudflare Worker secrets should hold FormBricks API credentials and Google/OIDC verification config.