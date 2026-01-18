-- Remove indexes from todos
DROP INDEX IF EXISTS idx_todos_priority;
DROP INDEX IF EXISTS idx_todos_decision;

-- Remove columns from todos
ALTER TABLE todos
    DROP COLUMN IF EXISTS tags,
    DROP COLUMN IF EXISTS priority,
    DROP COLUMN IF EXISTS decision_id;

-- Drop agent_decisions table and indexes
DROP TABLE IF EXISTS agent_decisions;

-- Drop enum types
DROP TYPE IF EXISTS todo_priority;
DROP TYPE IF EXISTS decision_status;
DROP TYPE IF EXISTS decision_type;
DROP TYPE IF EXISTS decision_source_type;
