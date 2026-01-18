#!/bin/bash
set -e

# DATABASE_URL is required
if [ -z "$DATABASE_URL" ]; then
  echo "ERROR: DATABASE_URL environment variable is not set"
  exit 1
fi

echo "Waiting for database to be ready..."
# Use psql with DATABASE_URL directly
until psql "$DATABASE_URL" -c '\q' 2>/dev/null; do
  echo "Database not ready, waiting..."
  sleep 2
done

echo "Running database migrations..."
# Run each migration SQL file in order
for migration in /app/migrations/*/up.sql; do
  echo "Running migration: $migration"
  psql "$DATABASE_URL" -f "$migration" 2>&1 || echo "Migration already applied or failed: $migration"
done

# Load seed data if it exists
if [ -f /app/seed-data.sql ]; then
  echo "Loading seed data..."
  psql "$DATABASE_URL" -f /app/seed-data.sql 2>&1 || echo "Seed data already loaded or failed"
fi

echo "Starting backend server..."
exec /app/backend
