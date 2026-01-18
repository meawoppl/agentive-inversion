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
- **Database**: Neon PostgreSQL with Diesel ORM
- **Purpose**: REST API for todo CRUD operations
- **Port**: 3000

### Shared Types (`crates/shared-types`)
- Common data structures used across all crates
- Includes models for todos, email accounts, and calendar accounts
- Serialization support for both API and database

### Email Poller (`crates/email-poller`)
- **Purpose**: Polls multiple Gmail accounts for new emails that should become todos
- **Interval**: Every 5 minutes
- **API**: Google Gmail API with OAuth2

### Calendar Poller (`crates/calendar-poller`)
- **Purpose**: Polls Google Calendar for upcoming events
- **Status**: Not yet implemented (stubbed out)
- **API**: Google Calendar API with OAuth2

## Data Flow

```
Gmail Accounts → Email Poller → Database ← Backend API ← Frontend
                                    ↑
Calendar APIs → Calendar Poller ────┘
```

## Database Schema

### Tables
- **todos**: Main todo items with source tracking
- **email_accounts**: Configured Gmail accounts
- **calendar_accounts**: Configured Google Calendar accounts

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
- PostgreSQL (Neon or local)
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
DATABASE_URL=postgres://user:password@db.neon.tech/agentive_inversion?sslmode=require
GMAIL_CLIENT_ID=your-client-id.apps.googleusercontent.com
GMAIL_CLIENT_SECRET=your-client-secret
GOOGLE_CALENDAR_CLIENT_ID=your-client-id.apps.googleusercontent.com
GOOGLE_CALENDAR_CLIENT_SECRET=your-client-secret
```

### 3. Database Setup

```bash
diesel migration run
```

### 4. Install WASM Target

```bash
rustup target add wasm32-unknown-unknown
```

## Development

### Run All Services Locally

Terminal 1 - Backend:
```bash
cargo run --bin backend
```

Terminal 2 - Frontend:
```bash
cd crates/frontend
trunk serve
```

Terminal 3 - Email Poller:
```bash
cargo run --bin email-poller
```

Terminal 4 - Calendar Poller:
```bash
cargo run --bin calendar-poller
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

- **Deploy** (`deploy.yml`): Runs on main branch
  - Backend deployment
  - Frontend deployment

## Project Structure

```
agentive-inversion/
├── .github/
│   └── workflows/          # GitHub Actions
├── crates/
│   ├── backend/           # Axum REST API
│   ├── frontend/          # Yew WASM app
│   ├── shared-types/      # Common types
│   ├── email-poller/      # Gmail integration
│   └── calendar-poller/   # Calendar integration
├── migrations/            # Diesel migrations
├── Cargo.toml            # Workspace config
├── diesel.toml           # Diesel config
└── .env.example          # Environment template
```

## Future Enhancements

- [ ] Calendar poller implementation (Google Calendar integration)
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
