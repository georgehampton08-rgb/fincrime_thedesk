-- FinCrime: The Desk — Migration 009: Customer Close Tracking

ALTER TABLE customer ADD COLUMN close_tick INTEGER;

CREATE INDEX IF NOT EXISTS idx_customer_close_tick
    ON customer (run_id, status, close_tick);
