-- Remove the category_id and link columns from todos
ALTER TABLE todos DROP COLUMN category_id;
ALTER TABLE todos DROP COLUMN link;

-- Drop the categories table
DROP TABLE categories;
