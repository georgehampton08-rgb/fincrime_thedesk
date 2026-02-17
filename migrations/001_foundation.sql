-- FinCrime: The Desk — Migration 001: Foundation
-- Phase 0 schema. Only tables the engine itself needs.
-- Subsystem tables added in later migrations.
CREATE TABLE IF NOT EXISTS run (
    run_id TEXT PRIMARY KEY,
    seed INTEGER NOT NULL,
    version TEXT NOT NULL,
    started_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS event_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    subsystem TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_event_log_tick ON event_log (run_id, tick);
CREATE TABLE IF NOT EXISTS sim_clock (
    run_id TEXT PRIMARY KEY,
    current_tick INTEGER NOT NULL DEFAULT 0,
    speed TEXT NOT NULL DEFAULT 'normal',
    paused INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE TABLE IF NOT EXISTS snapshot (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    state_json TEXT NOT NULL,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_snapshot_tick ON snapshot (run_id, tick);
CREATE TABLE IF NOT EXISTS player_command (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    cmd_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE TABLE IF NOT EXISTS macro_state (
    run_id TEXT PRIMARY KEY,
    tick INTEGER NOT NULL,
    base_rate REAL NOT NULL DEFAULT 0.05,
    economic_phase TEXT NOT NULL DEFAULT 'expansion',
    fraud_multiplier REAL NOT NULL DEFAULT 1.0,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);