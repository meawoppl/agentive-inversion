UPDATE connector_actions SET status='cancelled',error='Automatic archive delegation rolled back',updated_at=now()
WHERE autoarchive_json IS NOT NULL AND status IN ('proposed','queued');
UPDATE connector_connections SET scopes=array_remove(scopes,'actions:autoarchive');
ALTER TABLE connector_actions DROP COLUMN autoarchive_json;
