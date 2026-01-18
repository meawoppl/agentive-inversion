-- Agent decisions table for tracking all agent actions with full audit trail
CREATE TABLE agent_decisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Source information
    source_type VARCHAR(50) NOT NULL,  -- 'email', 'calendar', 'manual'
    source_id UUID,  -- FK to emails/calendar_events (nullable for manual)
    source_external_id VARCHAR(255),  -- Gmail ID / Google Event ID

    -- Decision details
    decision_type VARCHAR(50) NOT NULL,  -- 'create_todo', 'ignore', 'archive', 'defer', 'categorize', 'set_due_date'
    proposed_action TEXT NOT NULL,  -- JSON string of structured action data
    reasoning TEXT NOT NULL,  -- Human-readable explanation
    reasoning_details TEXT,  -- JSON string of structured reasoning data (keywords, scores, etc.)
    confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),

    -- Status tracking
    status VARCHAR(50) NOT NULL DEFAULT 'proposed',  -- 'proposed', 'approved', 'rejected', 'auto_approved', 'executed', 'failed'

    -- Related entities
    applied_rule_id UUID,  -- FK to agent_rules if rule-matched (will be added later)
    result_todo_id UUID REFERENCES todos(id) ON DELETE SET NULL,

    -- User feedback
    user_feedback TEXT,  -- User's correction/comment on rejection

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at TIMESTAMPTZ,  -- When user reviewed
    executed_at TIMESTAMPTZ  -- When action was executed
);

-- Indexes for common queries
CREATE INDEX idx_agent_decisions_status ON agent_decisions(status, created_at DESC);
CREATE INDEX idx_agent_decisions_source ON agent_decisions(source_type, source_id);
CREATE INDEX idx_agent_decisions_created ON agent_decisions(created_at DESC);
CREATE INDEX idx_agent_decisions_result_todo ON agent_decisions(result_todo_id);

-- Add decision_id to todos table to link back to the decision that created it
ALTER TABLE todos ADD COLUMN decision_id UUID REFERENCES agent_decisions(id) ON DELETE SET NULL;
CREATE INDEX idx_todos_decision ON todos(decision_id);
