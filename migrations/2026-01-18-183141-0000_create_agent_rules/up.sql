-- Agent rules table for automatic decision-making based on user-defined patterns
CREATE TABLE agent_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Rule identification
    name VARCHAR(255) NOT NULL,
    description TEXT,

    -- Scope
    source_type VARCHAR(50) NOT NULL,  -- 'email', 'calendar', 'any'
    rule_type VARCHAR(50) NOT NULL,  -- 'exact_match', 'contains', 'regex', 'sender', 'label', 'time_based'

    -- Rule definition (stored as JSON string, not JSONB)
    conditions TEXT NOT NULL,  -- JSON string of match conditions
    action VARCHAR(50) NOT NULL,  -- 'create_todo', 'ignore', 'archive', 'categorize', etc.
    action_params TEXT,  -- JSON string of action parameters (optional)

    -- Rule priority and status
    priority INTEGER NOT NULL DEFAULT 0,  -- Higher = evaluated first
    is_active BOOLEAN NOT NULL DEFAULT true,

    -- Provenance
    created_from_decision_id UUID,  -- FK to agent_decisions if rule was created from a decision

    -- Usage tracking
    match_count INTEGER NOT NULL DEFAULT 0,
    last_matched_at TIMESTAMPTZ,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for common queries
CREATE INDEX idx_agent_rules_active ON agent_rules(is_active, priority DESC);
CREATE INDEX idx_agent_rules_source_type ON agent_rules(source_type);
CREATE INDEX idx_agent_rules_rule_type ON agent_rules(rule_type);
CREATE INDEX idx_agent_rules_created_from ON agent_rules(created_from_decision_id);
