-- Track per-account sync health so failing accounts are visible and can back off
-- IF NOT EXISTS so a partially-applied/unrecorded run is safely re-runnable
ALTER TABLE google_accounts
    ADD COLUMN IF NOT EXISTS last_synced_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_sync_error TEXT,
    ADD COLUMN IF NOT EXISTS last_sync_error_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS consecutive_failures INTEGER NOT NULL DEFAULT 0;
