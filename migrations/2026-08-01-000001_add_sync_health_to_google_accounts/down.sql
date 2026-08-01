ALTER TABLE google_accounts
    DROP COLUMN last_synced_at,
    DROP COLUMN last_sync_error,
    DROP COLUMN last_sync_error_at,
    DROP COLUMN consecutive_failures;
