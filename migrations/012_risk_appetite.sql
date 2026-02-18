-- FinCrime: The Desk — Migration 012: Risk Appetite Controls

CREATE TABLE IF NOT EXISTS risk_appetite_state (
    run_id                  TEXT    NOT NULL,
    tick                    INTEGER NOT NULL,
    -- Dial values
    fee_aggressiveness      REAL    NOT NULL DEFAULT 1.0,
    growth_velocity         REAL    NOT NULL DEFAULT 1.0,
    service_level           REAL    NOT NULL DEFAULT 1.0,
    retention_spend         REAL    NOT NULL DEFAULT 1.0,
    compliance_stringency   REAL    NOT NULL DEFAULT 1.0,
    -- Risk profile
    overall_risk_score      REAL    NOT NULL DEFAULT 0.5,
    revenue_risk            REAL    NOT NULL DEFAULT 0.5,
    operational_risk        REAL    NOT NULL DEFAULT 0.5,
    compliance_risk         REAL    NOT NULL DEFAULT 0.5,
    financial_risk          REAL    NOT NULL DEFAULT 0.5,
    risk_level              TEXT    NOT NULL DEFAULT 'moderate',
    -- Violations
    comfort_zone_violations INTEGER NOT NULL DEFAULT 0,
    constraint_warnings     TEXT,
    PRIMARY KEY (run_id, tick)
);

CREATE TABLE IF NOT EXISTS dial_change_log (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    tick                INTEGER NOT NULL,
    dial_id             TEXT    NOT NULL,
    old_value           REAL    NOT NULL,
    new_value           REAL    NOT NULL,
    change_reason       TEXT,
    player_initiated    INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_dial_change_dial
    ON dial_change_log (run_id, dial_id, tick DESC);

CREATE TABLE IF NOT EXISTS board_pressure_event (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    tick                INTEGER NOT NULL,
    pressure_type       TEXT    NOT NULL,
    dial_id             TEXT    NOT NULL,
    message             TEXT    NOT NULL,
    severity            TEXT    NOT NULL,
    acknowledged        INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_board_pressure_unack
    ON board_pressure_event (run_id, acknowledged, tick DESC);

CREATE TABLE IF NOT EXISTS scenario_preview (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    tick_created        INTEGER NOT NULL,
    scenario_name       TEXT    NOT NULL,
    -- Proposed dial changes
    dial_changes        TEXT    NOT NULL,
    -- Projected impacts (30-tick forward)
    projected_revenue_delta REAL,
    projected_cost_delta REAL,
    projected_complaint_delta REAL,
    projected_churn_delta REAL,
    projected_risk_score REAL,
    -- Metadata
    created_by_player   INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

CREATE TABLE IF NOT EXISTS dial_impact_snapshot (
    run_id              TEXT    NOT NULL,
    tick                INTEGER NOT NULL,
    dial_id             TEXT    NOT NULL,
    -- Observed impacts (actual vs projected)
    revenue_impact      REAL    NOT NULL DEFAULT 0.0,
    cost_impact         REAL    NOT NULL DEFAULT 0.0,
    complaint_impact    REAL    NOT NULL DEFAULT 0.0,
    churn_impact        REAL    NOT NULL DEFAULT 0.0,
    -- Attribution confidence
    confidence_score    REAL    NOT NULL DEFAULT 0.5,
    PRIMARY KEY (run_id, tick, dial_id)
);
