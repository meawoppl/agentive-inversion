-- Restore the schema shape only; rule data is not recoverable.
ALTER TABLE agent_decisions ADD COLUMN IF NOT EXISTS applied_rule_id UUID;
CREATE TABLE IF NOT EXISTS agent_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    source_type VARCHAR(50) NOT NULL,
    rule_type VARCHAR(50) NOT NULL,
    conditions TEXT NOT NULL,
    action VARCHAR(50) NOT NULL,
    action_params TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_from_decision_id UUID,
    match_count INTEGER NOT NULL DEFAULT 0,
    last_matched_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
