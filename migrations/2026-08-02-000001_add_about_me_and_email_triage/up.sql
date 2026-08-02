-- Personal context document the triage agent reads before proposing todos.
-- Stored in the database because the repo is public; single-row table.
CREATE TABLE IF NOT EXISTS about_me (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    content TEXT NOT NULL DEFAULT '',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO about_me (id, content) VALUES (1, '') ON CONFLICT (id) DO NOTHING;

-- Pipeline state for the multi-model triage flow. Emails stay 'pending' until
-- the screening pass runs; terminal states record what the pipeline did.
ALTER TABLE emails
    ADD COLUMN IF NOT EXISTS triage_status VARCHAR NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS triaged_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_emails_triage_pending
    ON emails (fetched_at) WHERE triage_status = 'pending';
