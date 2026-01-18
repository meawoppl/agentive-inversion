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
# Create diesel migrations table if it doesn't exist
psql "$DATABASE_URL" -c "
CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (
    version VARCHAR(50) PRIMARY KEY,
    run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);" 2>/dev/null || true

# Run each migration that hasn't been applied yet
for migration_dir in /app/migrations/*/; do
  if [ -d "$migration_dir" ]; then
    version=$(basename "$migration_dir")

    # Check if migration has already been applied
    if ! psql "$DATABASE_URL" -t -c "SELECT 1 FROM __diesel_schema_migrations WHERE version = '$version'" 2>/dev/null | grep -q 1; then
      echo "Applying migration: $version"

      if psql "$DATABASE_URL" -f "$migration_dir/up.sql" 2>&1; then
        # Record successful migration
        psql "$DATABASE_URL" -c "INSERT INTO __diesel_schema_migrations (version) VALUES ('$version');" 2>/dev/null
        echo "  Applied: $version"
      else
        echo "  FAILED: $version"
        exit 1
      fi
    else
      echo "  Skipped (already applied): $version"
    fi
  fi
done

echo "Migrations complete."
echo "Starting backend server..."
exec /app/backend
