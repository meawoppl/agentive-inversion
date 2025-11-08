# Agentive Inversion - Self-Updating Todo List

A sophisticated, self-updating todo list application that automatically aggregates tasks from multiple sources including Gmail and Google Calendar. Built entirely in Rust with a modern WebAssembly frontend.

## Overview

Agentive Inversion is a smart todo list that doesn't just manage your tasks—it discovers them. By integrating with your Gmail accounts and Google Workspace calendars, it automatically creates, updates, and organizes todos based on your emails and scheduled events.

### Key Features

- **Self-Updating**: Automatically polls multiple Gmail accounts and Google Calendar instances
- **Unified Dashboard**: Single interface to view tasks from all sources
- **Real-time Updates**: WebAssembly-powered frontend with instant synchronization
- **Secure & Scalable**: Built on Neon SQL (PostgreSQL) with robust authentication
- **Intelligent Parsing**: Extracts actionable items from emails and calendar events

## Architecture

### Technology Stack

- **Frontend**: Yew (Rust WebAssembly framework) + Trunk (build tool)
- **Backend**: Axum (async web framework) + Diesel ORM
- **Database**: Neon SQL (serverless PostgreSQL)
- **Polling Services**: Tokio-based async services for Gmail/Calendar integration
- **Testing**: GitHub Actions CI/CD pipeline

### Component Structure

```
agentive-inversion/
├── frontend/           # Yew WebAssembly application
├── backend/            # Axum REST API server
├── polling-services/   # Background polling services
├── shared/             # Shared types and models
└── migrations/         # Diesel database migrations
```

### Data Flow

```
Gmail/Calendar APIs → Polling Services → Database ← Backend API ← Frontend
                            ↓
                      Task Processing
                      & Intelligence
```

## Getting Started

### Prerequisites

- Rust 1.75+ (with `wasm32-unknown-unknown` target)
- PostgreSQL 15+ or Neon SQL account
- Trunk (`cargo install trunk`)
- Diesel CLI (`cargo install diesel_cli --no-default-features --features postgres`)
- Google Cloud Platform project with Gmail and Calendar APIs enabled

### Environment Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/yourusername/agentive-inversion.git
   cd agentive-inversion
   ```

2. **Set up environment variables**
   ```bash
   cp .env.example .env
   # Edit .env with your configuration
   ```

   Required variables:
   ```env
   DATABASE_URL=postgresql://user:password@host/database
   GOOGLE_CLIENT_ID=your-client-id
   GOOGLE_CLIENT_SECRET=your-client-secret
   GOOGLE_REDIRECT_URI=http://localhost:8080/auth/callback
   ```

3. **Run database migrations**
   ```bash
   diesel migration run
   ```

4. **Install frontend dependencies**
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

### Development

**Run all services concurrently:**

```bash
# Terminal 1: Backend API
cd backend
cargo run

# Terminal 2: Frontend dev server
cd frontend
trunk serve

# Terminal 3: Polling services
cd polling-services
cargo run
```

**Run tests:**
```bash
cargo test --workspace
```

**Build for production:**
```bash
# Frontend
cd frontend
trunk build --release

# Backend
cargo build --release
```

## Project Components

### Frontend (`frontend/`)
- **Framework**: Yew 0.21 with Component-based architecture
- **Routing**: yew-router for SPA navigation
- **State**: Global state management with Yew contexts
- **API Client**: gloo-net for REST API communication

### Backend (`backend/`)
- **Web Framework**: Axum with tower middleware
- **ORM**: Diesel with async support (diesel-async)
- **Database Pool**: Deadpool for connection pooling
- **Authentication**: OAuth2 with Google Identity

### Polling Services (`polling-services/`)
- **Gmail Poller**: Monitors multiple Gmail accounts for actionable emails
- **Calendar Poller**: Syncs events from Google Workspace calendars
- **Task Processing**: Intelligent extraction of tasks from content
- **Scheduling**: Configurable polling intervals per account

### Shared (`shared/`)
- Common data models (Todo, Task, User, etc.)
- API request/response types
- Utility functions and traits

## API Endpoints

### Todos
- `GET /api/todos` - List all todos
- `GET /api/todos/:id` - Get specific todo
- `POST /api/todos` - Create new todo
- `PUT /api/todos/:id` - Update todo
- `DELETE /api/todos/:id` - Delete todo

### Sources
- `GET /api/sources` - List connected sources (Gmail/Calendar)
- `POST /api/sources/gmail` - Add Gmail account
- `POST /api/sources/calendar` - Add Calendar
- `DELETE /api/sources/:id` - Remove source

### Sync
- `POST /api/sync/trigger` - Manually trigger polling
- `GET /api/sync/status` - Get sync status

## Database Schema

```sql
todos
  - id: UUID (PK)
  - title: VARCHAR
  - description: TEXT
  - source_type: ENUM (gmail, calendar, manual)
  - source_id: VARCHAR
  - due_date: TIMESTAMP
  - completed: BOOLEAN
  - created_at: TIMESTAMP
  - updated_at: TIMESTAMP

sources
  - id: UUID (PK)
  - user_id: UUID (FK)
  - source_type: ENUM (gmail, calendar)
  - credentials: JSONB
  - polling_interval: INTEGER
  - last_polled: TIMESTAMP
  - enabled: BOOLEAN
```

## Configuration

### Polling Intervals
Configure in `polling-services/config.toml`:

```toml
[gmail]
default_interval_seconds = 300  # 5 minutes

[calendar]
default_interval_seconds = 600  # 10 minutes
```

### Google API Setup

1. Create a project in [Google Cloud Console](https://console.cloud.google.com)
2. Enable Gmail API and Google Calendar API
3. Create OAuth 2.0 credentials
4. Add authorized redirect URIs
5. Download credentials and add to `.env`

## Testing

Tests are automatically run via GitHub Actions on every push and pull request.

**Local testing:**
```bash
# Unit tests
cargo test

# Integration tests
cargo test --test '*'

# Frontend tests
cd frontend && wasm-pack test --headless --firefox
```

## CI/CD

GitHub Actions workflows:
- **CI** (`.github/workflows/ci.yml`): Runs tests, linting, and builds
- **Deploy** (`.github/workflows/deploy.yml`): Deploys to production (planned)

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Roadmap

- [ ] OAuth2 authentication flow
- [ ] Gmail integration with intelligent task extraction
- [ ] Google Calendar sync
- [ ] Task prioritization algorithm
- [ ] Natural language processing for task parsing
- [ ] Notification system
- [ ] Mobile-responsive UI
- [ ] Recurring task support
- [ ] Task dependencies and subtasks
- [ ] Export/import functionality