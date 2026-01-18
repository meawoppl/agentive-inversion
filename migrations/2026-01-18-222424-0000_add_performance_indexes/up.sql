-- Performance indexes for common query patterns

-- Email thread lookups (grouping emails by conversation)
CREATE INDEX idx_emails_thread ON emails(thread_id);

-- Composite index for active todos with due dates (partial index for active todos only)
CREATE INDEX idx_todos_active_due ON todos(completed, due_date) WHERE completed = false;

-- Calendar accounts active sync queries (partial index)
CREATE INDEX idx_calendar_accounts_active ON calendar_accounts(is_active) WHERE is_active = true;
