-- Revert OAuth and sync tracking additions
DROP INDEX IF EXISTS idx_email_accounts_sync_status;
DROP INDEX IF EXISTS idx_email_accounts_active;

ALTER TABLE email_accounts
DROP COLUMN IF EXISTS is_active,
DROP COLUMN IF EXISTS last_sync_error,
DROP COLUMN IF EXISTS sync_status,
DROP COLUMN IF EXISTS last_message_id,
DROP COLUMN IF EXISTS oauth_token_expires_at,
DROP COLUMN IF EXISTS oauth_access_token,
DROP COLUMN IF EXISTS oauth_refresh_token;
