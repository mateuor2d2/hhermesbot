-- Migration: Fix broadcasts table - add created_at column
-- Note: This migration was already applied manually
-- The column was added via: ALTER TABLE broadcasts ADD COLUMN created_at DATETIME;
-- and populated with: UPDATE broadcasts SET created_at = sent_at WHERE created_at IS NULL;

-- This file is now a no-op since the changes are already in place
SELECT 1;
