-- FinCrime: The Desk — Migration 011: Complaint Analytics

CREATE TABLE IF NOT EXISTS complaint_pattern (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    tick_detected       INTEGER NOT NULL,
    pattern_type        TEXT    NOT NULL,
    -- Pattern details
    issue_category      TEXT    NOT NULL,
    segment             TEXT,
    affected_count      INTEGER NOT NULL,
    window_start_tick   INTEGER NOT NULL,
    window_end_tick     INTEGER NOT NULL,
    -- Metrics
    velocity_ratio      REAL    NOT NULL,
    concentration_pct   REAL    NOT NULL,
    severity_score      REAL    NOT NULL,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_complaint_pattern_detection
    ON complaint_pattern (run_id, tick_detected DESC, severity_score DESC);

CREATE TABLE IF NOT EXISTS complaint_root_cause (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    complaint_id        TEXT    NOT NULL,
    root_cause_type     TEXT    NOT NULL,
    root_cause_id       TEXT,
    -- Attribution
    confidence_score    REAL    NOT NULL,
    correlation_lag_ticks INTEGER NOT NULL,
    -- Context
    attributed_tick     INTEGER NOT NULL,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_root_cause_complaint
    ON complaint_root_cause (run_id, complaint_id);
CREATE INDEX IF NOT EXISTS idx_root_cause_type
    ON complaint_root_cause (run_id, root_cause_type, confidence_score DESC);

CREATE TABLE IF NOT EXISTS resolution_effectiveness (
    run_id              TEXT    NOT NULL,
    resolution_code     TEXT    NOT NULL,
    measurement_tick    INTEGER NOT NULL,
    -- Effectiveness metrics
    avg_satisfaction_delta REAL NOT NULL,
    avg_churn_risk_delta REAL  NOT NULL,
    repeat_complaint_rate REAL NOT NULL,
    escalation_rate     REAL    NOT NULL,
    -- Sample size
    resolution_count    INTEGER NOT NULL,
    PRIMARY KEY (run_id, resolution_code, measurement_tick)
);

CREATE TABLE IF NOT EXISTS sla_performance_snapshot (
    run_id              TEXT    NOT NULL,
    tick                INTEGER NOT NULL,
    priority            TEXT    NOT NULL,
    -- Distribution
    aging_0_3_days      INTEGER NOT NULL DEFAULT 0,
    aging_4_7_days      INTEGER NOT NULL DEFAULT 0,
    aging_8_14_days     INTEGER NOT NULL DEFAULT 0,
    aging_15_30_days    INTEGER NOT NULL DEFAULT 0,
    aging_30_plus_days  INTEGER NOT NULL DEFAULT 0,
    -- SLA metrics
    total_open          INTEGER NOT NULL,
    at_risk_count       INTEGER NOT NULL,
    breach_count        INTEGER NOT NULL,
    breach_rate         REAL    NOT NULL,
    avg_age_ticks       REAL    NOT NULL,
    PRIMARY KEY (run_id, tick, priority)
);

CREATE TABLE IF NOT EXISTS repeat_complainer (
    run_id              TEXT    NOT NULL,
    customer_id         TEXT    NOT NULL,
    tick_flagged        INTEGER NOT NULL,
    complaint_count     INTEGER NOT NULL,
    -- Risk indicators
    total_unresolved    INTEGER NOT NULL,
    total_breached      INTEGER NOT NULL,
    avg_severity        REAL    NOT NULL,
    churn_risk          REAL    NOT NULL,
    -- Flags
    regulatory_risk_flag INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (run_id, customer_id, tick_flagged)
);
CREATE INDEX IF NOT EXISTS idx_repeat_complainer_risk
    ON repeat_complainer (run_id, regulatory_risk_flag DESC, complaint_count DESC);

CREATE TABLE IF NOT EXISTS complaint_cost_detail (
    run_id              TEXT    NOT NULL,
    tick                INTEGER NOT NULL,
    segment             TEXT    NOT NULL,
    -- Volume
    total_complaints    INTEGER NOT NULL DEFAULT 0,
    -- Cost breakdown
    handling_cost       REAL    NOT NULL DEFAULT 0.0,
    escalation_cost     REAL    NOT NULL DEFAULT 0.0,
    write_off_cost      REAL    NOT NULL DEFAULT 0.0,
    total_cost          REAL    NOT NULL DEFAULT 0.0,
    -- Efficiency
    cost_per_complaint  REAL    NOT NULL DEFAULT 0.0,
    PRIMARY KEY (run_id, tick, segment)
);

CREATE TABLE IF NOT EXISTS early_warning_alert (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    tick_fired          INTEGER NOT NULL,
    alert_type          TEXT    NOT NULL,
    severity            TEXT    NOT NULL,
    -- Details
    segment             TEXT,
    metric_name         TEXT    NOT NULL,
    current_value       REAL    NOT NULL,
    threshold_value     REAL    NOT NULL,
    delta_pct           REAL    NOT NULL,
    -- Status
    acknowledged        INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_early_warning_unack
    ON early_warning_alert (run_id, acknowledged, tick_fired DESC);
