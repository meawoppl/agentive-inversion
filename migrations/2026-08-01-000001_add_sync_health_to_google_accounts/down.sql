ALTER TABLE google_accounts
    DROP COLUMN IF EXISTS last_synced_at,
    DROP COLUMN IF EXISTS last_sync_error,
    DROP COLUMN IF EXISTS last_sync_error_at,
    DROP COLUMN IF EXISTS consecutive_failures;
