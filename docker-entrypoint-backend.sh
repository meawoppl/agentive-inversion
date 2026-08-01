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
# Create the migrations table, and widen the version column on databases
# created before it was VARCHAR(255) (CREATE TABLE IF NOT EXISTS won't alter
# an existing table)
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "
CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (
    version VARCHAR(255) PRIMARY KEY,
    run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
ALTER TABLE __diesel_schema_migrations ALTER COLUMN version TYPE VARCHAR(255);"

# Run each migration that hasn't been applied yet
for migration_dir in /app/migrations/*/; do
  if [ -d "$migration_dir" ]; then
    version=$(basename "$migration_dir")

    # Check if migration has already been applied
    if ! psql "$DATABASE_URL" -t -c "SELECT 1 FROM __diesel_schema_migrations WHERE version = '$version'" | grep -q 1; then
      echo "Applying migration: $version"

      if psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$migration_dir/up.sql"; then
        # Record successful migration; a failure here must be loud, since a
        # silently unrecorded migration re-runs on every restart
        psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "INSERT INTO __diesel_schema_migrations (version) VALUES ('$version');"
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
