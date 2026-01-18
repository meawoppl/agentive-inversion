-- Revert chat_messages table
DROP INDEX IF EXISTS idx_chat_messages_created_at;
DROP TABLE IF EXISTS chat_messages;
