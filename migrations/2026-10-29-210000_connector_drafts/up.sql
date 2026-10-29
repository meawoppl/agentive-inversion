-- No mail bodies or credentials: just a durable reservation/receipt for draft creation.
CREATE TABLE connector_drafts (
    id UUID PRIMARY KEY,
    connection_id UUID NOT NULL REFERENCES connector_connections(id),
    email_id UUID NOT NULL REFERENCES emails(id),
    request_key TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    gmail_draft_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(connection_id, request_key)
);
