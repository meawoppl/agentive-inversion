-- The keyword/rule classification system is removed: the agentic triage
-- pipeline is the only classifier. Idempotent (runs embedded on startup).
DROP TABLE IF EXISTS agent_rules;
ALTER TABLE agent_decisions DROP COLUMN IF EXISTS applied_rule_id;
