# Agentive Inversion

A self-updating todo list application built in Rust that automatically syncs tasks from multiple sources including Gmail accounts and Google Calendar.

## Architecture

This project uses a Rust workspace with the following crates:

### Frontend (`crates/frontend`)
- **Framework**: Yew (React-like framework for Rust/WASM)
- **Build Tool**: Trunk
- **Purpose**: Web UI for viewing and managing todos
- **Port**: 8080

### Backend (`crates/backend`)
- **Framework**: Axum (async web framework)
- **Database**: PostgreSQL with Diesel ORM (self-hosted Postgres 17 container in production)
- **Purpose**: REST API, plus background pollers (Gmail today, Calendar planned) that run as tokio tasks inside the server process
- **Port**: 3000

### Shared Types (`crates/shared-types`)
- Common data structures used across all crates
- Includes models for todos, google accounts, emails, calendar events, and agent decisions
- Serialization support for both API and database

## Data Flow

```
Gmail Accounts → Email Poller (in backend) → Database ← Backend API ← Frontend
```

Emails are classified by the agentic triage pipeline (Claude Code sessions
driven by the backend). Actionable ones become pending decisions in the
decision inbox; approving a decision creates a todo.

## Database Schema

### Tables
- **todos**: Main todo items with source tracking
- **google_accounts**: Connected Google accounts (OAuth refresh tokens for Gmail and Calendar)
- **emails**: Fetched email metadata and bodies
- **calendar_events**: Google Calendar events (poller not yet implemented)
- **categories**: Todo categories
- **agent_decisions**: Proposed actions awaiting review in the decision inbox
- **chat_messages**: Chat widget history

### Todo Sources
- `Manual`: User-created todos
- `Email`: Extracted from email
- `Calendar`: Calendar events converted to todos

## Quick Testing with Docker

Want to see the UI without setting up everything? Use Docker:

```bash
docker-compose up --build
```

Then open http://localhost:8080 in your browser. See [DOCKER.md](DOCKER.md) for details.

## Prerequisites

- Rust 1.70+ with `wasm32-unknown-unknown` target
- PostgreSQL (any instance; production uses a self-hosted Postgres 17 container)
- Diesel CLI: `cargo install diesel_cli --no-default-features --features postgres`
- Trunk: `cargo install trunk`
- Google Cloud project with Gmail and Calendar APIs enabled

Or use Docker for testing (see above).

## Setup

### 1. Clone and Setup Environment

```bash
git clone <repository-url>
cd agentive-inversion
cp .env.example .env
```

### 2. Configure Environment Variables

Edit `.env` with your configuration:

```bash
DATABASE_URL=postgres://user:password@localhost/agentive_inversion
GOOGLE_CLIENT_ID=your-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-client-secret
```

### 3. Database Setup

The server applies its embedded migrations on startup, so this is only needed
when iterating on migrations locally:

```bash
diesel migration run
```

### 4. Install WASM Target

```bash
rustup target add wasm32-unknown-unknown
```

## Development

### Build the Frontend First

`memory-serve` embeds `crates/frontend/dist` into the backend binary at compile
time, so the directory must exist before the backend will build:

```bash
cd crates/frontend && trunk build
```

### Run All Services Locally

Terminal 1 - Backend (serves the API, the embedded frontend, and the pollers on
port 3000). Add `--dev-mode` to skip starting the Gmail and Calendar pollers:
```bash
cargo run --bin backend
```

Terminal 2 - Frontend with live reload (optional; port 8080, proxies to the
backend):
```bash
cd crates/frontend
trunk serve
```

### Run Tests

```bash
cargo test --workspace
```

### Format Code

```bash
cargo fmt
```

### Lint

```bash
cargo clippy --workspace --all-features
```

## API Endpoints

### Todos
- `GET /api/todos` - List all todos
- `POST /api/todos` - Create a new todo
- `PUT /api/todos/:id` - Update a todo
- `DELETE /api/todos/:id` - Delete a todo

### Health Check
- `GET /health` - Service health status

## Google API Setup

### Gmail API
1. Create a project in Google Cloud Console
2. Enable Gmail API
3. Create OAuth 2.0 credentials
4. Add authorized redirect URIs
5. Download credentials and update `.env`

### Calendar API
1. Enable Google Calendar API in the same project
2. Use same OAuth 2.0 credentials or create separate ones
3. Update `.env` with credentials

## CI/CD

GitHub Actions workflows:
- **CI** (`ci.yml`): Runs on all PRs and pushes
  - Rust tests
  - Format check
  - Clippy linting
  - Frontend build
  - Database migration tests

- **Container** (`container.yml`): Runs on main branch and PRs
  - Builds backend binary and frontend dist
  - Pushes the combined image to `ghcr.io/meawoppl/agentive-inversion:main` on merge
  - Production watches that tag and auto-deploys the new image within ~5 minutes

## Project Structure

```
agentive-inversion/
├── .github/
│   └── workflows/          # GitHub Actions
├── crates/
│   ├── backend/           # Axum REST API + background pollers
│   ├── frontend/          # Yew WASM app
│   └── shared-types/      # Common types
├── migrations/            # Diesel migrations (embedded into the binary)
├── scripts/
│   └── check-migration-names.sh  # CI lint for migration directory names
├── Cargo.toml            # Workspace config
├── diesel.toml           # Diesel config
└── .env.example          # Environment template
```

## Future Enhancements

- [ ] AI-powered email parsing for better todo extraction
- [ ] Priority and categorization system
- [ ] Multi-user support
- [ ] Mobile responsive UI
- [ ] Real-time updates via WebSockets
- [ ] Todo completion tracking and analytics
- [ ] Recurring todos
- [ ] Due date reminders
- [ ] Task dependencies

## License

MIT
