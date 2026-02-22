-- Phase 3.6: Reputation Management tables
--
-- reputation_snapshot: one row per tick with the composite score.
-- reputation_event: individual driver events that changed the score.
CREATE TABLE IF NOT EXISTS reputation_snapshot (
    run_id TEXT NOT NULL REFERENCES run(run_id),
    tick INTEGER NOT NULL,
    score REAL NOT NULL,
    -- [0.0, 100.0]
    delta REAL NOT NULL,
    -- change vs previous tick
    PRIMARY KEY (run_id, tick)
);
CREATE TABLE IF NOT EXISTS reputation_event (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL REFERENCES run(run_id),
    tick INTEGER NOT NULL,
    driver TEXT NOT NULL,
    -- "exam_fine" | "mou" | "sar_late" | "sla_breach" | "recovery"
    delta REAL NOT NULL,
    -- signed impact (negative = bad)
    description TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reputation_event_run ON reputation_event(run_id, tick);