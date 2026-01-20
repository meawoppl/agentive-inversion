-- Update foreign keys to point to google_accounts instead of old tables
-- Then drop the old email_accounts and calendar_accounts tables

-- First, update emails.account_id to reference google_accounts
ALTER TABLE emails
    DROP CONSTRAINT IF EXISTS emails_account_id_fkey;

ALTER TABLE emails
    ADD CONSTRAINT emails_account_id_fkey
    FOREIGN KEY (account_id) REFERENCES google_accounts(id);

-- Update calendar_events.account_id to reference google_accounts
ALTER TABLE calendar_events
    DROP CONSTRAINT IF EXISTS calendar_events_account_id_fkey;

ALTER TABLE calendar_events
    ADD CONSTRAINT calendar_events_account_id_fkey
    FOREIGN KEY (account_id) REFERENCES google_accounts(id);

-- Drop the old tables
DROP TABLE IF EXISTS email_accounts;
DROP TABLE IF EXISTS calendar_accounts;
