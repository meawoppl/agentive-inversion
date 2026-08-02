-- Claude Code OAuth credential (single row) minted via the in-app
-- "Login to Claude Code" flow; lets the triage pipeline run on the
-- user's Claude subscription instead of an API key.
CREATE TABLE IF NOT EXISTS claude_credentials (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    oauth_token TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
