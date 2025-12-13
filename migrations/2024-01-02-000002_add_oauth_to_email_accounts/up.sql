-- Add OAuth credential storage and sync tracking to email_accounts table
ALTER TABLE email_accounts
ADD COLUMN oauth_refresh_token TEXT,
ADD COLUMN oauth_access_token TEXT,
ADD COLUMN oauth_token_expires_at TIMESTAMP WITH TIME ZONE,
ADD COLUMN last_message_id VARCHAR,
ADD COLUMN sync_status VARCHAR NOT NULL DEFAULT 'pending',
ADD COLUMN last_sync_error TEXT,
ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;

-- Add index for active accounts (used by poller)
CREATE INDEX idx_email_accounts_active ON email_accounts(is_active);

-- Add index for sync status
CREATE INDEX idx_email_accounts_sync_status ON email_accounts(sync_status);
