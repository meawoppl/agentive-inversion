#!/usr/bin/env bash
#
# Validates that all Diesel migration directories follow the naming convention:
#   - YYYY-MM-DD-HHMMSS_<description>  (standard timestamp format)
#
# Where <description> is lowercase snake_case (e.g., add_users_table)
#
# Migration directory names are load-bearing: Diesel derives the version it
# records in __diesel_schema_migrations from the name (everything before the
# first '_', with dashes stripped). Renaming an already-applied migration makes
# Diesel re-run it against a populated database, so the names below are frozen
# and grandfathered rather than fixed.
#
# Usage: ./scripts/check-migration-names.sh
# Exit code: 0 if all valid, 1 if any invalid

set -euo pipefail

MIGRATIONS_DIR="migrations"
ERRORS=0

# Pattern for valid migration names: YYYY-MM-DD-HHMMSS_<snake_case>
TIMESTAMP_PATTERN='^[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{6}_[a-z][a-z0-9_]*$'

# Names that predate this check and are already applied in production. They
# carry a stray "-0000" suffix on the timestamp. Do not add to this list —
# new migrations must match TIMESTAMP_PATTERN.
LEGACY_NAMES=(
    "2026-01-18-181300-0000_create_agent_decisions"
    "2026-01-18-183141-0000_create_agent_rules"
    "2026-01-18-201129-0000_create_chat_messages"
    "2026-01-18-212112-0000_create_calendar_events"
    "2026-01-18-222424-0000_add_performance_indexes"
    "2026-01-19-044025-0000_create_google_accounts"
)

is_legacy() {
    local candidate="$1"
    for legacy in "${LEGACY_NAMES[@]}"; do
        [[ "$candidate" == "$legacy" ]] && return 0
    done
    return 1
}

echo "Checking migration naming convention..."
echo "Expected format: YYYY-MM-DD-HHMMSS_snake_case_description"
echo "---"

for dir in "$MIGRATIONS_DIR"/*/; do
    # Skip if not a directory
    [[ -d "$dir" ]] || continue

    name=$(basename "$dir")

    # Skip hidden files/dirs
    [[ "$name" == .* ]] && continue

    if [[ "$name" =~ $TIMESTAMP_PATTERN ]]; then
        echo "  OK: $name"
    elif is_legacy "$name"; then
        echo "  OK (grandfathered): $name"
    else
        echo "  ERROR: $name"
        echo "         Expected format: YYYY-MM-DD-HHMMSS_snake_case_description"
        echo "         Example: 2026-01-15-143022_add_users_table"
        ERRORS=$((ERRORS + 1))
    fi
done

echo "---"

if [[ $ERRORS -gt 0 ]]; then
    echo "FAILED: $ERRORS migration(s) have invalid names"
    echo ""
    echo "To fix, rename the migration directory to match the format:"
    echo "  YYYY-MM-DD-HHMMSS_snake_case_description"
    echo ""
    echo "Only rename migrations that have never been applied anywhere."
    echo ""
    exit 1
else
    echo "PASSED: All migrations follow naming convention"
    exit 0
fi
