-- Create chat_messages table for storing conversation history
CREATE TABLE chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role VARCHAR(20) NOT NULL CHECK (role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    intent VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for efficient history retrieval
CREATE INDEX idx_chat_messages_created_at ON chat_messages(created_at DESC);
