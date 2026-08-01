# Architecture Overview

## System Components

### 1. Frontend (Yew/WASM)
**Location**: `crates/frontend`
**Tech Stack**: Yew, WebAssembly, Trunk
**Port**: 8080 (dev server)

Responsibilities:
- Decision inbox: review, approve, or reject agent-proposed todos
- Todo list UI (create, update, delete, sort, categorize)
- Decision log, category manager, calendar views, chat widget
- Google login flow and connected-account status

### 2. Backend (Axum)
**Location**: `crates/backend`
**Tech Stack**: Axum, Tokio, Diesel (async), yup-oauth2
**Port**: 3000

Responsibilities:
- REST API under `/api` (todos, emails, decisions, rules, categories,
  calendar events, chat, google accounts)
- Google OAuth login with JWT cookie sessions and an email allowlist
- Serving the built frontend as static files (SPA fallback)
- **Email poller** (tokio background task): polls Gmail for every connected
  account, stores emails, classifies them via agent rules plus a keyword
  heuristic, and creates pending decisions
- **Calendar poller** (tokio background task): currently a stub

### 3. Shared Types
**Location**: `crates/shared-types`
**Tech Stack**: Serde, Diesel (behind a feature flag so it also compiles to WASM)

Responsibilities:
- Domain models: todos, google accounts, emails, calendar events,
  agent decisions, agent rules, chat messages
- API request/response types
- The rule engine used for email classification

## Data Flow

```
┌─────────────────┐
│  Gmail Accounts │  (one row per account in google_accounts)
└────────┬────────┘
         │ Gmail API (OAuth2 refresh tokens)
         v
┌───────────────────────────────┐
│  Backend (Axum)               │
│  ├── email poller task        │──► emails table
│  ├── classifier (rules +      │──► agent_decisions (pending)
│  │    keyword heuristic)      │
│  └── REST API ◄───────────────┼──── Frontend (Yew)
└──────────────┬────────────────┘        │
               v                         │ approve/reject
        ┌──────────────┐                 │ in decision inbox
        │  PostgreSQL  │◄────────────────┘
        └──────────────┘     approved decision → todo
```

## Database Schema

Tables (see `crates/backend/src/schema.rs` for authoritative definitions):

- **todos** — id, title, description, completed, source (varchar), source_id,
  due_date, link, category_id → categories, decision_id → agent_decisions
- **google_accounts** — id, email (unique), name, refresh_token, access_token,
  token_expires_at
- **emails** — id, account_id → google_accounts, gmail_id, thread_id, headers,
  subject/from/to/cc, snippet, body_text, body_html, labels, received_at,
  processed flags. UNIQUE(account_id, gmail_id)
- **calendar_events** — id, account_id → google_accounts, google_event_id,
  summary, times, recurrence, attendees, processed flags.
  UNIQUE(account_id, google_event_id)
- **categories** — id, name (unique), color
- **agent_decisions** — proposed actions with reasoning, confidence, and
  status (proposed / approved / rejected / auto_approved / executed)
- **agent_rules** — user-defined classification rules (conditions as JSON
  text, action, priority, match tracking)
- **chat_messages** — chat widget history

## Authentication

- Single Google OAuth flow grants login plus Gmail and Calendar scopes
  (`gmail.modify`, `calendar`).
- Callback verifies a CSRF state cookie, checks the `ALLOWED_EMAILS`
  allowlist, stores the refresh token in `google_accounts`, and sets an
  HttpOnly JWT cookie (7-day expiry, sliding refresh).
- Connecting an additional Gmail account = logging in with it once.
- Pollers authenticate to Google APIs with the stored refresh tokens via
  yup-oauth2.

## Deployment Architecture

### Development
- Frontend: `trunk serve` (port 8080, proxies `/api` to the backend)
- Backend + pollers: `cargo run --bin backend` (port 3000)
- Database: any PostgreSQL (local or docker-compose)

### Production
- Merge to main → `container.yml` pushes `ghcr.io/meawoppl/agentive-inversion:main`
- Watchtower on the production host polls that tag and redeploys automatically
  (~5 minutes from merge to live); there is no manual deploy step
- Single backend container serves the API, pollers, and built frontend behind
  a TLS-terminating reverse proxy
- Database: self-hosted PostgreSQL 17 container with nightly dumps to S3
- The container entrypoint runs pending migrations before starting the server,
  so migrations must be idempotent

## Security Considerations

1. **Authentication**: Google OAuth2 with CSRF state verification; JWT cookie
   sessions; email allowlist re-checked on every request
2. **API**: CORS restricted via `CORS_ALLOWED_ORIGINS` in production;
   `RUST_ENV=production` enables the `Secure` cookie flag
3. **Secrets**: Environment variables, never committed

## Technology Choices

### Why Rust?
- Type safety across entire stack
- Performance (backend and WASM frontend)
- Excellent async support

### Why Yew?
- Native Rust for frontend
- Component-based architecture
- Type-safe props

### Why Axum?
- Modern async web framework
- Type-safe routing
- Good ecosystem integration

### Why Diesel?
- Compile-time query checking
- Migration system
- Async support via diesel-async

