-- Drop calendar_events table
DROP TABLE IF EXISTS calendar_events;

-- Remove OAuth and sync tracking fields from calendar_accounts
ALTER TABLE calendar_accounts
DROP COLUMN IF EXISTS email_address,
DROP COLUMN IF EXISTS oauth_refresh_token,
DROP COLUMN IF EXISTS oauth_access_token,
DROP COLUMN IF EXISTS oauth_token_expires_at,
DROP COLUMN IF EXISTS sync_token,
DROP COLUMN IF EXISTS sync_status,
DROP COLUMN IF EXISTS last_sync_error,
DROP COLUMN IF EXISTS is_active;
