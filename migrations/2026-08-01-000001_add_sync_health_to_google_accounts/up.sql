-- Track per-account sync health so failing accounts are visible and can back off
ALTER TABLE google_accounts
    ADD COLUMN last_synced_at TIMESTAMPTZ,
    ADD COLUMN last_sync_error TEXT,
    ADD COLUMN last_sync_error_at TIMESTAMPTZ,
    ADD COLUMN consecutive_failures INTEGER NOT NULL DEFAULT 0;
