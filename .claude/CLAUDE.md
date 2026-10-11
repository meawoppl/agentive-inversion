# Agentive Inversion - Project Patterns

## Architecture Overview

This is a Rust workspace with 3 crates:
1. `backend` - Axum REST API server, plus Gmail ingestion and approval-gated connector effects running as tokio tasks in `crates/backend/src/pollers/`
2. `frontend` - Yew WASM application
3. `shared-types` - Common types shared across crates

## Development Patterns

### Cargo Dependency Management
- Use `cargo add` to add dependencies
- Never manually edit Cargo.toml for dependencies
- Workspace dependencies are defined in root Cargo.toml

### Database Patterns
- Use Diesel ORM for all database operations
- Migrations are in `/migrations`, embedded into the binary by
  `diesel_migrations::embed_migrations!` and applied on startup by
  `db::run_migrations()`. There is no migration step in the container
- Run `diesel migration run` after creating new migrations for local iteration
- **Migration directory names are load-bearing and immutable once applied.**
  Diesel derives the recorded version from the name: everything before the
  first `_`, with dashes stripped (`2024-01-01-000001_create_todos` ->
  `20240101000001`). Renaming an applied migration makes Diesel re-run it
  against a populated database
- New migrations must be named `YYYY-MM-DD-HHMMSS_snake_case_description`;
  `scripts/check-migration-names.sh` enforces this in CI. Six pre-existing
  names carry a stray `-0000` suffix and are grandfathered in that script — do
  not add to that list
- DATABASE_URL points to PostgreSQL (self-hosted postgres:17 container in production)
- Schema file is auto-generated at `crates/backend/src/schema.rs`
- **Avoid JSONB columns** - prefer TEXT columns with JSON strings, or better yet, create proper normalized tables
- If you need to store structured data, consider creating a separate table with proper columns instead of a JSON blob
- Only use JSON strings (TEXT) when the schema is truly dynamic or varies per-row

### Type System Patterns
- **Avoid `serde_json::Value`** - always create proper typed structs for serialization
- Use strongly typed data structures instead of untyped JSON
- Only use `serde_json::Value` at API boundaries when parsing/serializing, convert to typed structs immediately
- Define explicit types for all data structures in `shared-types` crate

### Linking Back to the Source Email
- Every surface that names a source email links to it. The permalink is built
  **server-side** by `gmail_permalink` and shipped as `gmail_link` on
  `DecisionEmailContext` and `ArchiveReviewItem` — the frontend must not
  reconstruct the URL and cannot: it never sees the `gmail_id`
- Gmail permalinks are per-mailbox (`/mail/u/<account>/`), so the link needs
  the account the message landed in. With several accounts connected, using
  the wrong one silently opens the wrong mailbox — join through
  `email.account_id`, never assume a single account
- Approving a `create_todo` decision carries the permalink onto the todo's
  `link`, which the Todos list already renders
- Subjects that double as links use `.email-link`: plain text until hovered,
  so a dense queue does not become a wall of blue. The standalone "open in
  Gmail" action uses `.source-email-link` and looks like a link. Subjects
  inside a click-to-open row must `stop_propagation`, or the click both
  navigates and opens the modal

### Dates in Decision Surfaces
- Proposal rows, group headers and the detail modal show the **email's
  received date**, not the decision's `created_at`. A proposal made today
  about a three-week-old message is a three-week-old thing, and dating it
  "today" hides the staleness the reviewer is judging. `created_at` is still
  shown, explicitly labelled "Proposed", in the detail modal and the tooltip
- The Decision Log is a chronology of decisions, so it keeps `created_at`

### Frontend Development
- Use Trunk to build and serve the frontend
- Run `trunk serve` from `crates/frontend` for development
- WASM target required: `rustup target add wasm32-unknown-unknown`
- All frontend code is in Yew components
- Styles are in `crates/frontend/styles.css`
- **`crates/frontend/dist` must exist before the backend compiles** —
  `memory-serve`'s `load_assets!` reads it at compile time. Run
  `cd crates/frontend && trunk build` first; CI does this before clippy and
  test
- **Rebuild the backend after every `trunk build`.** The asset manifest is
  baked in at compile time, so a backend binary built against an older `dist`
  serves the SPA fallback for the new hashed asset names. The browser then
  rejects `index.html`-as-JS on the subresource-integrity check and renders a
  blank page with no visible error except an SRI digest mismatch in the
  console. Symptom to recognise: identical computed SHA-384 digests reported
  for the .js and the .css

### Static Asset Serving
Use **`memory-serve`** for embedding and serving the frontend in the axum
server. It pre-compresses assets (brotli/gzip) at build time, and handles
content negotiation, ETag/304, cache-control, and the SPA fallback.

`memory-serve` 0.6.x is the axum-0.7 compatible line (2.x requires axum 0.8+).
Its transitive `brotli` 6 requires pinning `alloc-stdlib = "=0.2.2"` — see the
comment in `crates/backend/Cargo.toml`.

### Testing
- Run tests with `cargo test --workspace`
- Normal tests need no DB. The ignored connector contract test requires disposable PostgreSQL; CI provides it. Never run that test against production or a development database containing useful data.
- Router behaviour is tested in-process via `build_app()` +
  `tower::ServiceExt::oneshot` — no bound port, no network. See the tests at
  the bottom of `crates/backend/src/main.rs`
- Every shared type gets a serde roundtrip test. The enums carry two distinct
  string forms — JSON is the PascalCase variant name (`"CreateTodo"`), while
  `as_str()`/`parse()` are the snake_case database form (`"create_todo"`).
  Both are pinned in `serde_roundtrip_tests`
- GitHub Actions runs full test suite on PRs

### Code Quality
- Always run `cargo fmt` before committing
- Run `cargo clippy --workspace --all-targets` to check for issues
- CI runs with `RUSTFLAGS=-Dwarnings` and `--locked`, so warnings and a stale
  `Cargo.lock` both fail the build
- `cargo audit` runs in CI and currently passes with **no suppressions**. There
  is no `.cargo/audit.toml`; keep it that way. Fix the advisory or bump the
  dependency rather than adding an ignore list

### API Integration
- Gmail and Calendar APIs use OAuth2 via `yup-oauth2`
- Client credentials stored in environment variables
- Gmail sync runs independently; connector effects drain every five seconds.

## File Locations

### Configuration
- Environment variables: `.env` (not committed, use `.env.example` as template)
- Server config: `crates/backend/src/config.rs` — `Config::from_env()` resolves
  `HOST`, `PORT`, and `CORS_ALLOWED_ORIGINS`, applying defaults and logging
  every resolved value at startup. Add new server settings there rather than
  calling `env::var` at the point of use
- Diesel config: `diesel.toml`
- Trunk config: `crates/frontend/Trunk.toml`

### Source Code
- Backend router: `crates/backend/src/main.rs` — `build_app(state, config)` is a
  pure function returning the `Router`, kept separate from `main()` so tests can
  drive the app in-process
- Backend handlers: `crates/backend/src/handlers.rs`
- Database layer: `crates/backend/src/db.rs`
- Shared models: `crates/shared-types/src/lib.rs`
- Frontend entrypoint: `crates/frontend/src/main.rs`; app shell and feature
  components live in sibling modules (see ARCHITECTURE.md)

### CI/CD
- Test workflow: `.github/workflows/ci.yml`
- Container build/push: `.github/workflows/container.yml` (pushes `ghcr.io/meawoppl/agentive-inversion:main`)
- Deployment is automatic: watchtower on the production host polls the `:main` tag and redeploys within ~5 minutes of a merge. There is no deploy workflow; do not retag the image without coordinating with infrastructure. The `type=ref,event=branch` tag in `container.yml` is what produces `:main` — do not remove it
- The container runs the binary directly (`CMD ["/app/backend"]`) as a non-root user. The server applies embedded migrations on startup before binding a port; migrations must be idempotent (use `IF NOT EXISTS` / `IF EXISTS`)
- CI must build the frontend before the backend in every job that compiles the backend, because the asset embed happens at compile time
- Upsert conflict targets must match an actual unique constraint on the table (e.g. emails is UNIQUE(account_id, gmail_id), not gmail_id alone) — Postgres rejects the INSERT at runtime otherwise, and Diesel won't catch it at compile time

## Common Commands

### Development
```bash
(cd crates/frontend && trunk build)  # Required before the backend compiles
cargo run --bin backend            # Start backend server (includes pollers)
cargo run --bin backend -- --dev-mode  # ... without ingestion or effect execution
cd crates/frontend && trunk serve --proxy-backend http://localhost:3000/api/  # Live reload
```

### Testing
```bash
cargo test --workspace           # Run all tests
cargo test -p backend            # Run backend tests only
./scripts/check-migration-names.sh  # Validate migration directory names
cargo audit                      # Check advisories (must pass with no ignores)
```

### Database
```bash
diesel migration run             # Apply migrations
diesel migration revert          # Rollback last migration
diesel migration generate <name> # Create new migration
```

### Code Quality
```bash
cargo fmt                                       # Format code
cargo clippy --workspace --all-targets --locked # Lint code as CI does
```

## Type System Patterns

### Shared Types
- All domain models go in `shared-types` crate
- Use serde for serialization
- Diesel derives for database models
- Use `#[cfg_attr]` for conditional derives

### Error Handling
- Use `anyhow::Result` for application code
- Use `thiserror` for custom error types
- Return proper HTTP status codes in handlers

## Service Communication

### Backend API
- RESTful endpoints under `/api/`
- Health check at `/health`
- CORS enabled for frontend

### Poller Services
- Run as tokio background tasks inside the backend process
- Write directly to database
- Spawned from `crates/backend/src/main.rs`

### Portal connector

- Agents execute in the Git workspace via Portal cron. No hosted AI runtime.
- Project knowledge and tasks are Git-owned. Legacy project writes return 410.
- `connector/` implements pairing, account scopes, work leases, proposals, and receipts.
- Every provider effect requires a paired reviewer connection; website cookies and agent grants cannot approve.
- Never add implicit execution to a read, export, cron, or work-completion endpoint.
- Claims are fenced by ID/generation and connection; preserve replay semantics.
- Unknown send outcomes are never automatically retried. Preserve partial receipts.
- See `docs/CONNECTOR_API.md` for actual contracts and `ARCHITECTURE.md` for boundaries.

## Environment Variables

Required variables (see `.env.example`):
- `DATABASE_URL` - PostgreSQL connection string
- `GOOGLE_CLIENT_ID` - Google OAuth client ID (used for Gmail and Calendar APIs)
- `GOOGLE_CLIENT_SECRET` - Google OAuth secret
- `RUST_LOG` - Logging level (info, debug, etc.)

## Git Workflow Notes

- Branch naming: `meawoppl/feature-name`
- Keep commit messages concise
- Run `cargo fmt` before all commits
- GitHub Actions must pass before merging

## SHIP Workflow

When the user says **"SHIP"**, execute this workflow:

1. **Create PR**: `gh pr create` with appropriate title and body
2. **Watch CI**: Use `gh pr checks <PR_NUMBER> --watch` to actively monitor all checks
3. **Fix minor CI failures**: If `cargo fmt` or `clippy` fails, fix automatically and push
4. **Re-watch CI**: After any fix, run `gh pr checks <PR_NUMBER> --watch` again
5. **Merge when passing**: `gh pr merge <PR_NUMBER> --squash --delete-branch`
6. **Update local**: `git checkout main && git pull`

**Important**: Do NOT use `--auto` flag for merging. Actively watch CI with `gh pr checks --watch` so failures are caught immediately and can be fixed in the same session.

**CI Failure Handling**:
- **Minor issues** (formatting, clippy warnings): Fix automatically, commit, push, and re-watch CI
- **Substantive issues** (test failures, build errors): Ask the user before making changes

This is a shorthand for the full PR review cycle when the user is confident in the changes.
