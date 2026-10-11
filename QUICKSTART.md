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

Visit http://localhost:3000. Dev mode skips Gmail ingestion and the provider effect
worker, but still requires authentication and a database. Run without `--dev-mode`
to enable these tasks. Agent execution and review live in the separate private
workspace through Agent Portal; no AI credential is needed by this service.

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

Normal tests do not require a running database. CI additionally runs the ignored
connector contract test against disposable PostgreSQL (see README). The frontend build must precede backend
compilation because `memory-serve` reads its assets at compile time.

## Troubleshooting

- Database startup errors: confirm the database exists and `DATABASE_URL` is
  correct. A migration failure is logged and prevents the server from serving.
- Login errors: check the email allowlist, OAuth client, and exact callback URI.
- Missing `frontend/dist`: run the frontend build before compiling the backend.
- Blank page after rebuilding assets: rebuild and restart the backend too.
- No new proposals: check the Portal schedule and the workspace agent connection.

See [README.md](README.md), [ARCHITECTURE.md](ARCHITECTURE.md), and
[DOCKER.md](DOCKER.md) for more detail.
