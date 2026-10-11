# Agentive Inversion

A personal email-to-action application built entirely in Rust. It ingests mail
from connected Gmail accounts, classifies it with Claude Code sessions, and
presents proposed actions for review.

## What works today

- Gmail incremental sync and history backfill across connected accounts.
- Three-stage triage: Haiku screening, Sonnet archive determinations, and an
  Opus action pass informed by your About Me document.
- A grouped, paginated decision inbox and decision log with source-email links.
- Todo creation, completion, due dates, categories, and source tracking.
- Archive review bundles and unsubscribe actions. Archiving defaults to proposals;
  `TRIAGE_ARCHIVE_MODE=execute` enables automatic archive execution.
- Approval-gated receipt forwarding and event creation in the Google “Agent”
  calendar, with duplicate checks against connected calendars.
- Pipeline health, local timestamps, queued-event rescans, and re-triage controls.
- Google OAuth login, an email allowlist, and a chat widget with stored history
  and intent-based responses.

Calendar ingestion is still a stub: the calendar views read the local event
table, which the poller does not populate. Live duplicate checks and approved
calendar writes use Google directly. The app has a shared workspace; an email
allowlist does not provide separate per-user data.

## Architecture

| Crate | Responsibility |
| --- | --- |
| `crates/frontend` | Yew/WASM UI, built with Trunk, organized into feature modules |
| `crates/backend` | Axum API, authentication, PostgreSQL/Diesel, background ingestion and triage |
| `crates/shared-types` | Typed API contracts and domain models shared by both |

```text
Gmail → ingestion → PostgreSQL → triage → proposed decisions
                                             ↓ review
                                   todos / Gmail / Calendar
```

Ingestion and triage run independently: a classifier failure leaves emails
pending without stopping Gmail sync. Agents submit actions through `agent-cli`
and the backend REST API, rather than accessing the database directly.

The backend serves the built frontend and API on port 3000. Release builds embed
frontend assets and database migrations. Trunk optionally serves the UI on port
8080 during development.

See [ARCHITECTURE.md](ARCHITECTURE.md) for data flow and module responsibilities.

## Local setup

Use a current stable Rust toolchain (as CI does), the `wasm32-unknown-unknown`
target, Trunk (CI pins v0.21.14), PostgreSQL, and the native PostgreSQL client
libraries needed by Diesel. Diesel CLI is optional for migration development;
the server applies embedded migrations on startup.

From the repository root:

```bash
cp .env.example .env
rustup target add wasm32-unknown-unknown
```

Fill in `DATABASE_URL`, `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `JWT_SECRET`,
`ALLOWED_EMAILS`, and `AUTH_REDIRECT_URI`. Generate the JWT secret with
`openssl rand -hex 32`. Google login requires an OAuth web application client
with Gmail and Calendar APIs enabled and a matching callback URL, locally
`http://localhost:3000/api/auth/callback`.

```bash
# Build assets before compiling the backend.
(cd crates/frontend && trunk build)
cargo run --bin backend -- --dev-mode
```

Open http://localhost:3000. `--dev-mode` skips Gmail ingestion, triage, and the
calendar stub; login and database access are still required. Omit that flag to
start background work. Triage also needs `claude` and `agent-cli` on `PATH`, plus
a credential from the UI's Claude login or `ANTHROPIC_API_KEY`. Without a
credential, ingestion continues and triage stays disabled.

See [QUICKSTART.md](QUICKSTART.md) for development setup and
[DOCKER.md](DOCKER.md) for the container build requirements. The Dockerfile
copies prebuilt Linux binaries; it does not compile the application.

## Development checks

Build the frontend first, then run the same core checks as CI:

```bash
cargo fmt --all --check
RUSTFLAGS=-Dwarnings cargo clippy --workspace --all-targets --locked
cargo test --workspace --locked
./scripts/check-migration-names.sh
cargo audit
```

Tests do not require a live database. CI also builds the WASM frontend and
checks dependency advisories. Rebuild the backend after rebuilding embedded
assets so its asset manifest matches the frontend output.

## API and source map

The authoritative route list is `build_app` in `crates/backend/src/main.rs`.
Protected `/api` routes cover todos, categories, emails, decisions, pipeline
controls, calendar events, chat, Google accounts, About Me, and Claude login.
Google login routes and `/health` are public.

- `crates/backend/src/pollers/`: Gmail ingestion, triage orchestration, calendar stub.
- `crates/backend/src/services/`: decision execution, review grouping, triage policy,
  calendar duplicate checks, and unsubscribe handling.
- `crates/backend/src/auth/`: Google OAuth and JWT cookie sessions.
- `crates/backend/src/schema.rs`: authoritative database schema.
- `migrations/`: embedded Diesel migrations; applied names must never be changed.
- `crates/frontend/src/`: app shell, shared state, formatting, and feature components.

## Delivery

GitHub Actions checks formatting, migrations' names, tests, Clippy, and dependency
advisories. The container workflow builds the frontend, backend, and `agent-cli`,
and publishes `ghcr.io/meawoppl/agentive-inversion:main` on main-branch pushes.
The production host watches that tag for automatic deployment.

## License

[MIT](LICENSE).
