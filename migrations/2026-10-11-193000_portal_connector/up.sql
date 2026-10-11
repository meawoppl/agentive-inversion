CREATE TABLE IF NOT EXISTS connector_pairings (
 id UUID PRIMARY KEY, secret_hash TEXT NOT NULL UNIQUE, name TEXT NOT NULL, requested_review BOOLEAN NOT NULL DEFAULT false,
 expires_at TIMESTAMPTZ NOT NULL, connection_id UUID, created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS connector_connections (
 id UUID PRIMARY KEY, token_hash TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
 owner_email TEXT NOT NULL, account_ids UUID[] NOT NULL, scopes TEXT[] NOT NULL,
 expires_at TIMESTAMPTZ NOT NULL, revoked_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS connector_claims (
 email_id UUID PRIMARY KEY REFERENCES emails(id), claim_id UUID NOT NULL,
 connection_id UUID NOT NULL REFERENCES connector_connections(id),
 expires_at TIMESTAMPTZ NOT NULL, completed_at TIMESTAMPTZ,
 disposition TEXT, repository_commit TEXT, repository_path TEXT
);
CREATE INDEX IF NOT EXISTS connector_claim_batch ON connector_claims(claim_id);
CREATE TABLE IF NOT EXISTS connector_actions (
 id UUID PRIMARY KEY, connection_id UUID NOT NULL REFERENCES connector_connections(id),
 email_id UUID NOT NULL REFERENCES emails(id), idempotency_key TEXT NOT NULL,
 request_json TEXT NOT NULL, kind TEXT NOT NULL,
 status TEXT NOT NULL CHECK (status IN ('proposed','queued','running','succeeded','unknown','rejected','cancelled')),
 approved_by TEXT, approved_connection_id UUID REFERENCES connector_connections(id), forward_to TEXT, result_json TEXT, error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(connection_id, idempotency_key)
);
CREATE INDEX IF NOT EXISTS connector_action_queue ON connector_actions(status, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS connector_one_effect ON connector_actions(email_id,kind) WHERE status NOT IN ('rejected','cancelled');
