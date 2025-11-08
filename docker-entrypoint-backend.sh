#!/bin/bash
set -e

echo "Waiting for postgres..."
until PGPASSWORD=testpassword psql -h "postgres" -U "testuser" -d "agentive_inversion" -c '\q' 2>/dev/null; do
  sleep 1
done

echo "Running database migrations manually..."
# Run each migration SQL file
for migration in /app/migrations/*/up.sql; do
  echo "Running migration: $migration"
  PGPASSWORD=testpassword psql -h "postgres" -U "testuser" -d "agentive_inversion" -f "$migration" || echo "Migration already applied or failed: $migration"
done

echo "Loading seed data..."
PGPASSWORD=testpassword psql -h "postgres" -U "testuser" -d "agentive_inversion" -f /app/seed-data.sql || echo "Seed data already loaded or failed"

echo "Starting backend server..."
exec /app/backend
