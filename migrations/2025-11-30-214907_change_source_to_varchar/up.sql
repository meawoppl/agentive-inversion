-- Change source column from enum to VARCHAR
ALTER TABLE todos
  ALTER COLUMN source TYPE VARCHAR
  USING source::TEXT;
