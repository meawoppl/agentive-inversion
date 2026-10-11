# Docker Development Environment

Compose runs PostgreSQL 15 and one application container serving the frontend
and API on http://localhost:3000. Authentication is required. It does not load
sample data, and it starts the backend's background tasks by default.

## Build requirements

The Dockerfile copies `target/release/backend`.
It must be **Linux binaries compatible with the image architecture**, with
frontend assets embedded in the backend. Building native macOS binaries and
copying them into this image will not work.

The container workflow in `.github/workflows/container.yml` builds Linux x86-64
artifacts on Ubuntu. To build locally, use a compatible Linux environment with
Rust, Trunk, the WASM target, and PostgreSQL development libraries installed:

```bash
(cd crates/frontend && trunk build --release)
cargo build --release --locked --bin backend
```

Run the Docker build from a checkout containing those artifacts. Repeat both
steps after source changes; `docker compose up --build` alone only rebuilds the
image around the existing binaries.

## Configure and start

```bash
cp .env.example .env
```

Set `JWT_SECRET`, `ALLOWED_EMAILS`, `GOOGLE_CLIENT_ID`, and
`GOOGLE_CLIENT_SECRET`. Compose requires all four. Register the callback
`http://localhost:3000/api/auth/callback` in the Google OAuth web application
client. Compose provides that callback by default and overrides `DATABASE_URL`
to point at its PostgreSQL service.

```bash
docker compose up --build
```

The database health check runs first. The application then applies its embedded
migrations before serving requests.

- Application: http://localhost:3000
- Health: http://localhost:3000/health
- PostgreSQL: localhost:5432, database `agentive_inversion`

The local database credentials are `testuser` / `testpassword`; replace them
for deployments outside local development. Production uses PostgreSQL 17 and
a TLS reverse proxy, separately from this Compose file.

## Connector configuration

The image contains no agent runtime. Portal cron runs agents in the workspace.
The server ingests mail and executes explicitly approved provider effects.
Set `TRIAGE_FORWARD_TO` in the app environment to choose the receipt destination.
To disable background work locally, override the command with
`["/app/backend", "--dev-mode"]` in a Compose override file.

## Routine commands

```bash
# Logs
docker compose logs -f app
docker compose logs -f postgres

# Inspect the database
docker compose exec postgres psql -U testuser -d agentive_inversion

# Stop containers, retaining database data
docker compose down

# Start only PostgreSQL (also usable for native backend development)
docker compose up -d postgres
```

To reset the database deliberately, `docker compose down -v` removes its volume.
The next startup recreates the schema through migrations.

`seed-data.sql` is a legacy fixture that references the removed `email_accounts`
and `calendar_accounts` tables; it is not compatible with the current schema.
Manage tasks in the Git workspace and connect accounts through Google login instead.

## Troubleshooting

- Missing binary during image build: build the backend release binary first.
- `exec format error`: the binary architecture or operating system does not
  match the Linux image. Use compatible Linux build artifacts.
- App exits on startup: inspect `docker compose logs app` for migration or
  authentication configuration errors.
- Port collision: change the host side of the Compose port mapping. If the
  application origin changes, update `AUTH_REDIRECT_URI` and Google's allowed
  callback as well.
- Old UI after a rebuild: rebuild the frontend, then the backend, then the image.

See [QUICKSTART.md](QUICKSTART.md) for native development and
[DOCKER_QUICKREF.md](DOCKER_QUICKREF.md) for a compact command reference.
