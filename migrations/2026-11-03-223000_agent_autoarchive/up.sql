ALTER TABLE connector_actions ADD COLUMN autoarchive_json TEXT;
-- This deployment enables the owner's requested archive-only delegation for agent grants.
UPDATE connector_connections SET scopes=array_append(scopes,'actions:autoarchive')
WHERE 'work:write'=ANY(scopes) AND 'actions:propose'=ANY(scopes)
AND NOT ('actions:autoarchive'=ANY(scopes));
