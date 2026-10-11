# Quick Start Guide

Run commands from the repository root unless stated otherwise.

## Prerequisites

- Current stable Rust with Cargo, rustfmt, and Clippy.
- Trunk (CI uses v0.21.14) and the WASM target:

  ```bash
  rustup target add wasm32-unknown-unknown
  ```

- PostgreSQL and its native client development libraries (`libpq`).
- Google OAuth web application credentials with Gmail and Calendar APIs enabled.

The server runs migrations automatically. Install Diesel CLI only if you need
to create, inspect, or revert migrations during development.

## Configure and run

```bash
cp .env.example .env
```

Edit `.env` with your database connection and authentication settings:

| Variable | Value |
| --- | --- |
| `DATABASE_URL` | Connection to an existing PostgreSQL database |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | Google OAuth web application credentials |
| `JWT_SECRET` | Generate with `openssl rand -hex 32` |
| `ALLOWED_EMAILS` | Comma-separated Google account emails permitted to log in |
| `AUTH_REDIRECT_URI` | `http://localhost:3000/api/auth/callback` |

Register that exact callback in the OAuth client's authorized redirect URIs.
Login connects the Google account; do not insert refresh tokens manually.

```bash
(cd crates/frontend && trunk build)
cargo run --bin backend -- --dev-mode
```

Visit http://localhost:3000. Dev mode skips all three background tasks (Gmail,
triage, and the calendar stub), but still requires authentication and a database.

## Enable ingestion and triage

Run the backend without `--dev-mode` to enable background tasks. For triage,
install the Claude Code CLI and put the project's agent CLI on `PATH`:

```bash
cargo build --locked --bin agent-cli
export PATH="$PWD/target/debug:$PATH"
cargo run --bin backend
```

Use the UI's Claude login or set `ANTHROPIC_API_KEY`. Without credentials,
emails still ingest and remain pending. Archive decisions default to review;
see `.env.example` for execution policy, forwarding destination, and model
settings. Calendar polling is not implemented, although event proposals can
be checked against Google calendars and approved events can be created there.

## Frontend live reload

With the backend running on port 3000, use another terminal:

```bash
cd crates/frontend
trunk serve --proxy-backend http://localhost:3000/api/
```

Visit http://localhost:8080. The proxy sends relative `/api` requests to the
backend. OAuth returns to the callback configured in `.env` (port 3000 by
default); return to port 8080 after login to use live reload.

When serving built assets through the backend, rebuild it after every
`trunk build`; the compiled asset manifest must match the files in `dist`.

## Before committing

```bash
(cd crates/frontend && trunk build)
cargo fmt --all --check
RUSTFLAGS=-Dwarnings cargo clippy --workspace --all-targets --locked
cargo test --workspace --locked
./scripts/check-migration-names.sh
cargo audit
```

Tests do not require a running database. The frontend build must precede backend
compilation because `memory-serve` reads its assets at compile time.

## Troubleshooting

- Database startup errors: confirm the database exists and `DATABASE_URL` is
  correct. A migration failure is logged and prevents the server from serving.
- Login errors: check the email allowlist, OAuth client, and exact callback URI.
- Missing `frontend/dist`: run the frontend build before compiling the backend.
- Blank page after rebuilding assets: rebuild and restart the backend too.
- Triage disabled: check the Claude credential and that both `claude` and
  `agent-cli` are available to the backend process.

See [README.md](README.md), [ARCHITECTURE.md](ARCHITECTURE.md), and
[DOCKER.md](DOCKER.md) for more detail.
