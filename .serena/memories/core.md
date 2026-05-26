# Core

- Rust backend for jakarta.awscommunity.id, intended to deploy on Cloudflare Workers and proxy FormBricks APIs for volunteer/speaker application workflows.
- Current repo is a minimal scaffold: root-level `main.rs`, `Cargo.toml`, `README.md`; no module tree, no Wrangler config, and `main.rs` is empty at onboarding time.
- Read `mem:tech_stack` for language/build metadata, `mem:conventions` for current structure constraints, `mem:suggested_commands` for local commands, and `mem:task_completion` for completion checks.
- Read `mem:architecture/formbricks_application_plan` before implementing the FormBricks application workflow; it captures the agreed decisions for FormBricks-hosted per-division forms, D1 form registry scope, live response discovery by email, LinkedIn duplicate detection, Cloudflare Worker layering, and implementation sequence.