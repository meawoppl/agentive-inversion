DROP INDEX IF EXISTS idx_emails_triage_pending;
ALTER TABLE emails
    DROP COLUMN IF EXISTS triage_status,
    DROP COLUMN IF EXISTS triaged_at;
DROP TABLE IF EXISTS about_me;
