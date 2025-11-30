-- Create categories table
CREATE TABLE categories (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR NOT NULL UNIQUE,
    color VARCHAR,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Add link and category_id columns to todos table
ALTER TABLE todos ADD COLUMN link VARCHAR;
ALTER TABLE todos ADD COLUMN category_id UUID REFERENCES categories(id) ON DELETE SET NULL;

-- Add index for category lookups
CREATE INDEX idx_todos_category_id ON todos(category_id);

-- Add some default categories
INSERT INTO categories (name, color) VALUES
    ('Work', '#3b82f6'),
    ('Personal', '#10b981'),
    ('Important', '#ef4444'),
    ('Later', '#8b5cf6');
