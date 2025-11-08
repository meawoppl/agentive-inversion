# Docker Quick Reference

## One-Command Start

```bash
docker-compose up --build
```

Then visit: **http://localhost:8080**

## Common Commands

```bash
# Start (using Makefile)
make docker-up

# Stop
make docker-down

# Reset everything (fresh start)
make docker-clean
make docker-up

# View logs
make docker-logs
```

## What You Get

- ✅ PostgreSQL database with test data
- ✅ Backend API running on port 3000
- ✅ Frontend UI running on port 8080
- ✅ 10 sample todos (manual, email, calendar sources)
- ✅ Sample email and calendar accounts

## Test Data Included

**Manual Todos:**
- Buy groceries (due in 2 days)
- Finish project documentation (due in 1 week)
- Call dentist (due in 3 days)
- Review pull requests (completed)
- Update dependencies (due in 5 days)

**Email-sourced Todos:**
- Respond to client inquiry (due in 1 day)
- Submit expense report (due in 4 days)

**Calendar-sourced Todos:**
- Team standup meeting (tomorrow at 9:30 AM)
- Quarterly planning session (next week)
- Coffee chat with mentor (in 3 days at 3 PM)

## Troubleshooting

**Port 8080 already in use?**
```bash
# Change the port in docker-compose.yml
services:
  frontend:
    ports:
      - "9090:8080"  # Use port 9090 instead
```

**Want to see what's in the database?**
```bash
docker-compose exec postgres psql -U testuser -d agentive_inversion

# Then run queries:
SELECT * FROM todos;
SELECT * FROM email_accounts;
```

**Services won't start?**
```bash
# Clean everything
docker-compose down -v
docker system prune -a

# Start fresh
docker-compose up --build
```

## No Security Warning

This Docker setup is for **testing the UI only**:
- Hardcoded passwords (testuser/testpassword)
- No HTTPS
- No authentication
- CORS fully open

**DO NOT USE IN PRODUCTION**

## For Full Development

See [QUICKSTART.md](QUICKSTART.md) for setting up:
- Real Neon PostgreSQL database
- Google OAuth for Gmail/Calendar
- Individual service development
