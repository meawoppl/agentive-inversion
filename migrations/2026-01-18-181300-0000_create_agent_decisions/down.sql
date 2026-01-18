-- Remove decision_id from todos
DROP INDEX IF EXISTS idx_todos_decision;
ALTER TABLE todos DROP COLUMN IF EXISTS decision_id;

-- Drop agent_decisions table and indexes
DROP INDEX IF EXISTS idx_agent_decisions_result_todo;
DROP INDEX IF EXISTS idx_agent_decisions_created;
DROP INDEX IF EXISTS idx_agent_decisions_source;
DROP INDEX IF EXISTS idx_agent_decisions_status;
DROP TABLE IF EXISTS agent_decisions;
