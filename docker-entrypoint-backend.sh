#!/bin/bash
set -e

# Check that DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
  echo "ERROR: DATABASE_URL environment variable is not set"
  exit 1
fi

echo "Waiting for database to be ready..."
until psql "$DATABASE_URL" -c '\q' 2>/dev/null; do
  echo "Database not ready, waiting..."
  sleep 2
done

echo "Running database migrations..."
# Use diesel_cli for proper migration tracking
# This tracks which migrations have been applied in __diesel_schema_migrations table
diesel migration run --migration-dir /app/migrations

echo "Starting backend server..."
exec /app/backend
