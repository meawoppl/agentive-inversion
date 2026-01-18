-- Add OAuth and sync tracking fields to calendar_accounts
ALTER TABLE calendar_accounts
ADD COLUMN email_address VARCHAR(255),
ADD COLUMN oauth_refresh_token TEXT,
ADD COLUMN oauth_access_token TEXT,
ADD COLUMN oauth_token_expires_at TIMESTAMPTZ,
ADD COLUMN sync_token TEXT,
ADD COLUMN sync_status VARCHAR(50) NOT NULL DEFAULT 'pending',
ADD COLUMN last_sync_error TEXT,
ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT true;

-- Create calendar_events table for storing fetched events
CREATE TABLE calendar_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES calendar_accounts(id) ON DELETE CASCADE,
    google_event_id VARCHAR(255) NOT NULL,
    ical_uid VARCHAR(255),
    summary TEXT,
    description TEXT,
    location TEXT,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    all_day BOOLEAN NOT NULL DEFAULT false,
    recurring BOOLEAN NOT NULL DEFAULT false,
    recurrence_rule TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'confirmed',
    organizer_email VARCHAR(255),
    attendees TEXT, -- JSON array stored as text
    conference_link TEXT,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed BOOLEAN NOT NULL DEFAULT false,
    processed_at TIMESTAMPTZ,
    UNIQUE(account_id, google_event_id)
);

-- Indexes for efficient queries
CREATE INDEX idx_calendar_events_account ON calendar_events(account_id);
CREATE INDEX idx_calendar_events_time ON calendar_events(start_time, end_time);
CREATE INDEX idx_calendar_events_processed ON calendar_events(processed, start_time);
CREATE INDEX idx_calendar_events_google_id ON calendar_events(google_event_id);
