CREATE TABLE emails (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES email_accounts(id) ON DELETE CASCADE,
    gmail_id VARCHAR(255) NOT NULL,
    thread_id VARCHAR(255) NOT NULL,
    history_id BIGINT,
    subject TEXT NOT NULL,
    from_address VARCHAR(255) NOT NULL,
    from_name VARCHAR(255),
    to_addresses TEXT[] NOT NULL,
    cc_addresses TEXT[],
    snippet TEXT,
    body_text TEXT,
    body_html TEXT,
    labels TEXT[],
    has_attachments BOOLEAN NOT NULL DEFAULT FALSE,
    received_at TIMESTAMPTZ NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed BOOLEAN NOT NULL DEFAULT FALSE,
    processed_at TIMESTAMPTZ,
    archived_in_gmail BOOLEAN NOT NULL DEFAULT FALSE,
    UNIQUE(account_id, gmail_id)
);

CREATE INDEX idx_emails_processed ON emails(processed, fetched_at);
CREATE INDEX idx_emails_received ON emails(received_at DESC);
CREATE INDEX idx_emails_account ON emails(account_id);
