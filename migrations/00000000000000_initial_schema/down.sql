-- Drop triggers
DROP TRIGGER IF EXISTS set_todo_completed_at ON todos;
DROP TRIGGER IF EXISTS update_todos_updated_at ON todos;
DROP TRIGGER IF EXISTS update_sources_updated_at ON sources;
DROP TRIGGER IF EXISTS update_users_updated_at ON users;

-- Drop functions
DROP FUNCTION IF EXISTS set_completed_at();
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Drop tables
DROP TABLE IF EXISTS sync_logs;
DROP TABLE IF EXISTS todos;
DROP TABLE IF EXISTS sources;
DROP TABLE IF EXISTS users;

-- Drop custom types
DROP TYPE IF EXISTS todo_status;
DROP TYPE IF EXISTS priority;
DROP TYPE IF EXISTS source_type;
