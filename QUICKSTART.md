# Quick Start Guide

## Prerequisites

Install the following tools:

```bash
# Rust and cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# WASM target for frontend
rustup target add wasm32-unknown-unknown

# Diesel CLI for database migrations
cargo install diesel_cli --no-default-features --features postgres

# Trunk for building the frontend
cargo install trunk
```

## Setup Steps

### 1. Clone and Navigate

```bash
cd agentive-inversion
```

### 2. Set Up Environment

```bash
cp .env.example .env
# Edit .env with your actual values:
# - DATABASE_URL: Your Neon PostgreSQL connection string
# - Gmail and Calendar API credentials from Google Cloud Console
```

### 3. Run Database Migrations

```bash
diesel migration run
```

### 4. Start All Services

Open 4 terminal windows:

**Terminal 1 - Backend:**
```bash
cargo run --bin backend
# Backend will start on http://localhost:3000
```

**Terminal 2 - Frontend:**
```bash
cd crates/frontend
trunk serve
# Frontend will start on http://localhost:8080
```

**Terminal 3 - Email Poller:**
```bash
cargo run --bin email-poller
# Polls Gmail every 5 minutes
```

**Terminal 4 - Calendar Poller:**
```bash
cargo run --bin calendar-poller
# Polls Google Calendar every 5 minutes
```

### 5. Access the Application

Open your browser to: http://localhost:8080

## Development Workflow

### Making Changes

1. Edit code in your preferred editor
2. The frontend will auto-reload with Trunk's hot reload
3. Backend requires restart: Ctrl+C and re-run `cargo run --bin backend`

### Before Committing

```bash
# Format code
cargo fmt

# Check for issues
cargo clippy --workspace --all-features

# Run tests
cargo test --workspace
```

## Google API Setup

### Gmail API

1. Go to [Google Cloud Console](https://console.cloud.google.com)
2. Create a new project or select existing
3. Enable Gmail API
4. Create OAuth 2.0 credentials (Desktop app)
5. Download credentials JSON
6. Add client ID and secret to `.env`

### Calendar API

1. In the same Google Cloud project
2. Enable Google Calendar API
3. Use same OAuth credentials or create new ones
4. Add credentials to `.env`

## Troubleshooting

### Database Connection Issues
```bash
# Test connection string
psql $DATABASE_URL

# Verify migrations
diesel migration list
```

### Frontend Build Issues
```bash
# Clear trunk cache
rm -rf crates/frontend/dist

# Rebuild
cd crates/frontend && trunk build
```

### Dependency Issues
```bash
# Clean and rebuild
cargo clean
cargo build
```

## Next Steps

- Configure email accounts in the database
- Set up calendar sync
- Customize todo parsing logic
- Deploy to production

See [README.md](README.md) for full documentation.
See [ARCHITECTURE.md](ARCHITECTURE.md) for system design details.
