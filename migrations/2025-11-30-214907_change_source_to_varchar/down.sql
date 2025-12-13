-- Recreate the enum type
CREATE TYPE todo_source_type AS ENUM ('manual', 'email', 'calendar');

-- Change source column back to enum
ALTER TABLE todos
  ALTER COLUMN source TYPE todo_source_type
  USING source::todo_source_type;
