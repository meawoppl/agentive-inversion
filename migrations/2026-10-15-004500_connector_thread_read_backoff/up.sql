ALTER TABLE connector_claims ADD COLUMN read_retry_after TIMESTAMPTZ, ADD COLUMN last_read_status INTEGER;
