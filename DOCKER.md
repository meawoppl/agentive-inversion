# Docker Testing Environment

This guide explains how to run the entire application stack using Docker for quick testing and development.

## Overview

The Docker setup includes:
- **PostgreSQL 15**: Database with test data
- **App**: Single container serving both API and frontend

Everything runs in containers with no external dependencies needed.

## Prerequisites

Install Docker and Docker Compose:
- [Docker Desktop](https://www.docker.com/products/docker-desktop/) (includes Docker Compose)
- Or Docker Engine + Docker Compose plugin

## Quick Start

### 1. Start Everything

```bash
docker-compose up --build
```

This will:
1. Start PostgreSQL database
2. Build and start the app (runs migrations + seeds test data)

### 2. Access the Application

Once all services are running:
- **Application**: http://localhost:3000
- **Health Check**: http://localhost:3000/health
- **API**: http://localhost:3000/api/*

### 3. Stop Everything

```bash
# Stop and remove containers
docker-compose down

# Stop and remove containers + volumes (deletes database data)
docker-compose down -v
```

## Test Data

The setup automatically seeds the database with sample data:

**Sample Todos:**
- 5 manual todos (including completed items)
- 2 email-sourced todos
- 3 calendar-sourced todos

**Sample Accounts:**
- 2 email accounts (work@example.com, personal@example.com)
- 2 calendar accounts

You can modify `seed-data.sql` to customize the test data.

## Development Workflow

### Rebuilding After Code Changes

```bash
# Rebuild and restart
docker-compose up --build
```

### View Logs

```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f app
docker-compose logs -f postgres
```

### Access Database Directly

```bash
# Connect to PostgreSQL
docker-compose exec postgres psql -U testuser -d agentive_inversion

# Example queries
SELECT * FROM todos;
SELECT * FROM email_accounts;
SELECT * FROM calendar_accounts;
```

### Reset Database

```bash
# Stop containers and remove volumes
docker-compose down -v

# Start fresh (will re-run migrations and seed data)
docker-compose up --build
```

## Architecture

```
┌─────────────────────────────────────┐
│         Docker Network              │
│                                     │
│  ┌─────────────────────────────┐   │
│  │           App               │   │
│  │  (Axum backend + frontend)  │   │
│  │          :3000              │   │
│  │                             │   │
│  │  /api/* → API handlers      │   │
│  │  /*     → Static frontend   │   │
│  └─────────────┬───────────────┘   │
│                │                    │
│          ┌─────▼──────┐            │
│          │            │            │
│          │ PostgreSQL │            │
│          │   :5432    │            │
│          │            │            │
│          └────────────┘            │
│                                     │
└─────────────────────────────────────┘
         │
         │ Port Mappings
         ▼
    Host Machine
    localhost:3000 → App (API + Frontend)
    localhost:5432 → Database
```

## Configuration

### Environment Variables

The Docker setup uses hardcoded test credentials (defined in `docker-compose.yml`):
- Database: `testuser:testpassword`
- Database Name: `agentive_inversion`

**Note**: This is for testing only. DO NOT use these credentials in production.

### Customizing Ports

Edit `docker-compose.yml` to change port mappings:

```yaml
services:
  app:
    ports:
      - "8080:3000"  # Change host port to 8080
```

## Troubleshooting

### Port Already in Use

If port 3000 or 5432 is already in use:

```bash
# Find what's using the port
lsof -i :3000
lsof -i :5432

# Kill the process or change ports in docker-compose.yml
```

### Build Failures

```bash
# Clean everything and rebuild
docker-compose down -v
docker system prune -a
docker-compose up --build
```

### App Won't Start

Check if migrations are failing:
```bash
docker-compose logs app
```

Common issues:
- Database not ready: Wait for postgres health check
- Migration errors: Check `migrations/` directory

## Differences from Production

This Docker setup is for **testing only**:

1. **No Security**:
   - No HTTPS
   - Hardcoded credentials
   - No authentication

2. **No Gmail/Calendar Pollers**:
   - Requires real OAuth credentials
   - Not needed for UI testing

3. **Development Mode**:
   - More verbose logging
   - No optimizations

## Next Steps

Once you've tested the UI:
1. Set up real database (Neon PostgreSQL)
2. Configure Google OAuth credentials
3. Run services individually for development
4. See [QUICKSTART.md](QUICKSTART.md) for full setup

## Running Individual Services

You can run services independently:

```bash
# Just the database
docker-compose up postgres

# Database + App
docker-compose up postgres app

# Everything
docker-compose up
```

## Cleaning Up

```bash
# Remove containers
docker-compose down

# Remove containers + volumes
docker-compose down -v

# Remove containers + volumes + images
docker-compose down -v --rmi all
```
