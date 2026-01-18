-- Remove performance indexes
DROP INDEX IF EXISTS idx_emails_thread;
DROP INDEX IF EXISTS idx_todos_active_due;
DROP INDEX IF EXISTS idx_calendar_accounts_active;
