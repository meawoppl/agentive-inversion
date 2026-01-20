-- Recreate the old tables (data will be lost)
CREATE TABLE email_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_name VARCHAR NOT NULL,
    email_address VARCHAR NOT NULL,
    provider VARCHAR NOT NULL DEFAULT 'gmail',
    last_synced TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    oauth_refresh_token TEXT,
    oauth_access_token TEXT,
    oauth_token_expires_at TIMESTAMPTZ,
    last_message_id VARCHAR,
    sync_status VARCHAR NOT NULL DEFAULT 'pending',
    last_sync_error TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE calendar_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_name VARCHAR NOT NULL,
    calendar_id VARCHAR NOT NULL,
    last_synced TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    email_address VARCHAR,
    oauth_refresh_token TEXT,
    oauth_access_token TEXT,
    oauth_token_expires_at TIMESTAMPTZ,
    sync_token TEXT,
    sync_status VARCHAR NOT NULL DEFAULT 'pending',
    last_sync_error TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true
);

-- Restore foreign keys to old tables
ALTER TABLE emails
    DROP CONSTRAINT IF EXISTS emails_account_id_fkey;

ALTER TABLE emails
    ADD CONSTRAINT emails_account_id_fkey
    FOREIGN KEY (account_id) REFERENCES email_accounts(id);

ALTER TABLE calendar_events
    DROP CONSTRAINT IF EXISTS calendar_events_account_id_fkey;

ALTER TABLE calendar_events
    ADD CONSTRAINT calendar_events_account_id_fkey
    FOREIGN KEY (account_id) REFERENCES calendar_accounts(id);
