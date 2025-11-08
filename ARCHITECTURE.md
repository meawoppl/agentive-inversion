# Architecture Overview

## System Components

### 1. Frontend (Yew/WASM)
**Location**: `crates/frontend`
**Tech Stack**: Yew, WebAssembly, Trunk
**Port**: 8080 (dev server)

Responsibilities:
- Render todo list UI
- Handle user interactions (create, update, delete todos)
- Communicate with backend REST API
- Real-time UI updates

### 2. Backend (Axum)
**Location**: `crates/backend`
**Tech Stack**: Axum, Tokio, Diesel
**Port**: 3000

Responsibilities:
- REST API for todo operations
- Database access layer
- Authentication (future)
- Data validation

API Endpoints:
```
GET    /health              - Health check
GET    /api/todos           - List all todos
POST   /api/todos           - Create new todo
PUT    /api/todos/:id       - Update todo
DELETE /api/todos/:id       - Delete todo
```

### 3. Email Poller Service
**Location**: `crates/email-poller`
**Tech Stack**: async-imap, Google Gmail API, OAuth2

Responsibilities:
- Poll multiple Gmail accounts every 5 minutes
- Parse emails for actionable items
- Create todos in database
- Track last sync time per account

### 4. Calendar Poller Service
**Location**: `crates/calendar-poller`
**Tech Stack**: Google Calendar API, OAuth2

Responsibilities:
- Poll Google Calendar events every 5 minutes
- Extract upcoming events
- Create/update todos from events
- Sync event changes

### 5. Shared Types
**Location**: `crates/shared-types`
**Tech Stack**: Serde, Diesel

Responsibilities:
- Common data structures
- Serialization/deserialization
- Database models
- API request/response types

## Data Flow

```
┌─────────────────┐
│  Gmail Accounts │
└────────┬────────┘
         │
         │ (Gmail API)
         │
         v
┌──────────────────┐
│  Email Poller    │
│  (every 5 min)   │
└────────┬─────────┘
         │
         │
         v
    ┌────────────────────┐
    │                    │
    │   PostgreSQL       │◄─────┐
    │   (Neon DB)        │      │
    │                    │      │
    └────────┬───────────┘      │
             │                  │
             │                  │
         ┌───▼──────┐      ┌────┴─────────┐
         │          │      │              │
         │ Backend  │◄─────┤  Frontend    │
         │ (Axum)   │      │  (Yew)       │
         │          │      │              │
         └──────────┘      └──────────────┘
             ▲
             │
         ┌───┴─────────┐
         │             │
         │  Calendar   │
         │  Poller     │
         │ (every 5min)│
         └──────▲──────┘
                │
                │ (Calendar API)
                │
      ┌─────────┴──────────┐
      │  Google Calendars  │
      └────────────────────┘
```

## Database Schema

### todos table
```sql
id              UUID PRIMARY KEY
title           VARCHAR NOT NULL
description     TEXT
completed       BOOLEAN DEFAULT FALSE
source          todo_source_type NOT NULL
source_id       VARCHAR
due_date        TIMESTAMP WITH TIME ZONE
created_at      TIMESTAMP WITH TIME ZONE
updated_at      TIMESTAMP WITH TIME ZONE
```

Indexes:
- `idx_todos_completed` on `completed`
- `idx_todos_due_date` on `due_date`
- `idx_todos_source` on `source`

### email_accounts table
```sql
id              UUID PRIMARY KEY
account_name    VARCHAR NOT NULL
email_address   VARCHAR NOT NULL UNIQUE
provider        VARCHAR NOT NULL
last_synced     TIMESTAMP WITH TIME ZONE
created_at      TIMESTAMP WITH TIME ZONE
```

### calendar_accounts table
```sql
id              UUID PRIMARY KEY
account_name    VARCHAR NOT NULL
calendar_id     VARCHAR NOT NULL UNIQUE
last_synced     TIMESTAMP WITH TIME ZONE
created_at      TIMESTAMP WITH TIME ZONE
```

### Custom Types
```sql
CREATE TYPE todo_source_type AS ENUM ('manual', 'email', 'calendar');
```

## Service Communication

### Frontend ↔ Backend
- Protocol: HTTP/REST
- Format: JSON
- CORS enabled for local development

### Pollers → Database
- Direct database access via Diesel
- No HTTP layer needed
- Independent processes

### External APIs
- Gmail API: OAuth2 authentication
- Calendar API: OAuth2 authentication
- Credentials stored in environment variables

## Deployment Architecture

### Development
- Frontend: `trunk serve` (port 8080)
- Backend: `cargo run --bin backend` (port 3000)
- Email Poller: `cargo run --bin email-poller`
- Calendar Poller: `cargo run --bin calendar-poller`
- Database: Neon PostgreSQL (cloud)

### Production (Future)
- Frontend: Static files served via CDN
- Backend: Container deployment
- Pollers: Background services/cron jobs
- Database: Neon PostgreSQL with connection pooling

## Security Considerations

1. **Authentication**: OAuth2 for Google services
2. **Database**: SSL/TLS connections to Neon
3. **API**: CORS configuration for production
4. **Secrets**: Environment variables, never committed
5. **Tokens**: Stored securely, auto-refresh

## Scaling Considerations

1. **Database**: Connection pooling via diesel-async
2. **Pollers**: Rate limiting to respect API quotas
3. **Backend**: Stateless design for horizontal scaling
4. **Frontend**: WASM bundle size optimization

## Technology Choices

### Why Rust?
- Type safety across entire stack
- Performance (backend and WASM frontend)
- Excellent async support
- Strong ecosystem

### Why Yew?
- Native Rust for frontend
- Component-based architecture
- Type-safe props
- Great developer experience

### Why Axum?
- Modern async web framework
- Excellent ergonomics
- Type-safe routing
- Good ecosystem integration

### Why Diesel?
- Compile-time query checking
- Migration system
- Async support via diesel-async
- Type-safe schema

### Why Neon?
- Serverless PostgreSQL
- Automatic scaling
- Built-in connection pooling
- Great for development and production
