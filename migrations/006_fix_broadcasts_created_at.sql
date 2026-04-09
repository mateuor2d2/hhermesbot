-- Migration: Fix broadcasts table schema
-- The table was using 'sent_at' but the code expects 'created_at'

-- Add created_at column if it doesn't exist
ALTER TABLE broadcasts ADD COLUMN created_at DATETIME DEFAULT CURRENT_TIMESTAMP;

-- Copy sent_at values to created_at where created_at is NULL
UPDATE broadcasts SET created_at = sent_at WHERE created_at IS NULL;
