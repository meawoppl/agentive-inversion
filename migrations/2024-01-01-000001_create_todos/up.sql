CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TYPE todo_source_type AS ENUM ('manual', 'email', 'calendar');

CREATE TABLE todos (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR NOT NULL,
    description TEXT,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    source todo_source_type NOT NULL DEFAULT 'manual',
    source_id VARCHAR,
    due_date TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_todos_completed ON todos(completed);
CREATE INDEX idx_todos_due_date ON todos(due_date);
CREATE INDEX idx_todos_source ON todos(source);

CREATE TABLE email_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    account_name VARCHAR NOT NULL,
    email_address VARCHAR NOT NULL UNIQUE,
    provider VARCHAR NOT NULL,
    last_synced TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE calendar_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    account_name VARCHAR NOT NULL,
    calendar_id VARCHAR NOT NULL UNIQUE,
    last_synced TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
