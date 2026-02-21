-- Migration 023: Add customer names
-- Add name column to customer table for realistic customer identification

ALTER TABLE customer ADD COLUMN name TEXT DEFAULT '';

-- Create index for name lookups
CREATE INDEX IF NOT EXISTS idx_customer_name ON customer(name);
