-- Create enum types for decision fields
CREATE TYPE decision_source_type AS ENUM ('email', 'calendar', 'manual');
CREATE TYPE decision_type AS ENUM ('create_todo', 'ignore', 'archive', 'defer', 'delegate');
CREATE TYPE decision_status AS ENUM ('proposed', 'approved', 'rejected', 'auto_approved', 'executed');
CREATE TYPE todo_priority AS ENUM ('low', 'medium', 'high', 'urgent');

-- Create agent_decisions table
CREATE TABLE agent_decisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Source information
    source_type decision_source_type NOT NULL,
    source_id UUID NOT NULL,

    -- Decision details
    decision_type decision_type NOT NULL,
    proposed_action JSONB NOT NULL,
    reasoning TEXT NOT NULL,
    confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),

    -- Status tracking
    status decision_status NOT NULL DEFAULT 'proposed',

    -- Result tracking
    result_todo_id UUID REFERENCES todos(id) ON DELETE SET NULL,

    -- User feedback for rejections
    user_feedback TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    decided_at TIMESTAMPTZ,
    executed_at TIMESTAMPTZ
);

-- Indexes for common queries
CREATE INDEX idx_decisions_status ON agent_decisions(status);
CREATE INDEX idx_decisions_source ON agent_decisions(source_type, source_id);
CREATE INDEX idx_decisions_created ON agent_decisions(created_at DESC);
CREATE INDEX idx_decisions_pending ON agent_decisions(status, created_at) WHERE status = 'proposed';

-- Add new columns to todos table
ALTER TABLE todos
    ADD COLUMN decision_id UUID REFERENCES agent_decisions(id) ON DELETE SET NULL,
    ADD COLUMN priority todo_priority NOT NULL DEFAULT 'medium',
    ADD COLUMN tags TEXT[] DEFAULT '{}';

CREATE INDEX idx_todos_decision ON todos(decision_id);
CREATE INDEX idx_todos_priority ON todos(priority);
