# Agentive Inversion - Project Patterns

## Architecture Overview

This is a Rust workspace with 5 crates:
1. `backend` - Axum REST API server
2. `frontend` - Yew WASM application
3. `shared-types` - Common types shared across crates
4. `email-poller` - Gmail polling service
5. `calendar-poller` - Google Calendar polling service

## Development Patterns

### Cargo Dependency Management
- Use `cargo add` to add dependencies
- Never manually edit Cargo.toml for dependencies
- Workspace dependencies are defined in root Cargo.toml

### Database Patterns
- Use Diesel ORM for all database operations
- Migrations are in `/migrations`
- Run `diesel migration run` after creating new migrations
- Database URL points to Neon PostgreSQL
- Schema file is auto-generated at `crates/backend/src/schema.rs`

### Frontend Development
- Use Trunk to build and serve the frontend
- Run `trunk serve` from `crates/frontend` for development
- WASM target required: `rustup target add wasm32-unknown-unknown`
- All frontend code is in Yew components
- Styles are in `crates/frontend/styles.css`

### Testing
- Run tests with `cargo test --workspace`
- Database tests require DATABASE_URL environment variable
- GitHub Actions runs full test suite on PRs

### Code Quality
- Always run `cargo fmt` before committing
- Run `cargo clippy --workspace --all-features` to check for issues
- CI enforces fmt and clippy checks

### API Integration
- Gmail and Calendar APIs use OAuth2 via `yup-oauth2`
- Client credentials stored in environment variables
- Polling services run every 5 minutes

## File Locations

### Configuration
- Environment variables: `.env` (not committed, use `.env.example` as template)
- Diesel config: `diesel.toml`
- Trunk config: `crates/frontend/Trunk.toml`

### Source Code
- Backend handlers: `crates/backend/src/handlers.rs`
- Database layer: `crates/backend/src/db.rs`
- Shared models: `crates/shared-types/src/lib.rs`
- Frontend components: `crates/frontend/src/main.rs`

### CI/CD
- Test workflow: `.github/workflows/ci.yml`
- Deploy workflow: `.github/workflows/deploy.yml`

## Common Commands

### Development
```bash
cargo run --bin backend          # Start backend server
cd crates/frontend && trunk serve  # Start frontend dev server
cargo run --bin email-poller     # Start email poller
cargo run --bin calendar-poller  # Start calendar poller
```

### Testing
```bash
cargo test --workspace           # Run all tests
cargo test -p backend            # Run backend tests only
```

### Database
```bash
diesel migration run             # Apply migrations
diesel migration revert          # Rollback last migration
diesel migration generate <name> # Create new migration
```

### Code Quality
```bash
cargo fmt                        # Format code
cargo clippy --workspace         # Lint code
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
- Run as separate processes
- Write directly to database
- No HTTP communication needed
- Independent retry logic

## Environment Variables

Required variables (see `.env.example`):
- `DATABASE_URL` - Neon PostgreSQL connection string
- `GMAIL_CLIENT_ID` - Google OAuth client ID
- `GMAIL_CLIENT_SECRET` - Google OAuth secret
- `GOOGLE_CALENDAR_CLIENT_ID` - Calendar API client ID
- `GOOGLE_CALENDAR_CLIENT_SECRET` - Calendar API secret
- `RUST_LOG` - Logging level (info, debug, etc.)

## Git Workflow Notes

- Branch naming: `meawoppl/feature-name`
- Keep commit messages concise
- Run `cargo fmt` before all commits
- GitHub Actions must pass before merging
