-- FinCrime: The Desk — Migration 002: Macro State
-- Phase 1A: adds macro_state and player_command tables.
-- (player_command was scaffolded in 001 but gets its
--  index here for query performance.)
CREATE TABLE IF NOT EXISTS macro_state (
    run_id TEXT PRIMARY KEY,
    tick INTEGER NOT NULL,
    base_rate REAL NOT NULL DEFAULT 0.05,
    economic_phase TEXT NOT NULL DEFAULT 'expansion',
    fraud_multiplier REAL NOT NULL DEFAULT 1.0,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_player_command_tick ON player_command (run_id, tick);