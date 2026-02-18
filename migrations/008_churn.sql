-- FinCrime: The Desk — Migration 008: Advanced Churn Modeling

CREATE TABLE IF NOT EXISTS customer_churn_score (
    run_id                TEXT    NOT NULL,
    customer_id           TEXT    NOT NULL,
    tick                  INTEGER NOT NULL,
    churn_risk            REAL    NOT NULL,
    -- Component scores (for debugging/analysis)
    base_rate             REAL    NOT NULL,
    satisfaction_component REAL   NOT NULL,
    fee_burden_component  REAL    NOT NULL,
    complaint_component   REAL    NOT NULL,
    sla_breach_component  REAL    NOT NULL,
    inactivity_component  REAL    NOT NULL,
    product_depth_bonus   REAL    NOT NULL,
    retention_offer_bonus REAL    NOT NULL,
    life_event_multiplier REAL    NOT NULL DEFAULT 1.0,
    -- Prediction metadata
    predicted_churn_30d   REAL    NOT NULL,
    predicted_churn_90d   REAL    NOT NULL,
    PRIMARY KEY (run_id, customer_id, tick),
    FOREIGN KEY (run_id)      REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);
CREATE INDEX IF NOT EXISTS idx_churn_score_risk
    ON customer_churn_score (run_id, tick, churn_risk DESC);

CREATE TABLE IF NOT EXISTS life_event (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id            TEXT    NOT NULL,
    customer_id       TEXT    NOT NULL,
    event_type        TEXT    NOT NULL,
    tick_occurred     INTEGER NOT NULL,
    tick_expires      INTEGER NOT NULL,
    active            INTEGER NOT NULL DEFAULT 1,
    churn_risk_delta  REAL    NOT NULL,
    -- Behavioral change flags (JSON for flexibility)
    behavioral_changes TEXT   NOT NULL,
    FOREIGN KEY (run_id)      REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);
CREATE INDEX IF NOT EXISTS idx_life_event_customer
    ON life_event (run_id, customer_id, active);
CREATE INDEX IF NOT EXISTS idx_life_event_expiry
    ON life_event (run_id, tick_expires, active);

CREATE TABLE IF NOT EXISTS churn_cohort (
    run_id            TEXT    NOT NULL,
    cohort_id         TEXT    NOT NULL,
    tick_churned      INTEGER NOT NULL,
    segment           TEXT    NOT NULL,
    -- Attributes at churn time
    tenure_ticks      INTEGER NOT NULL,
    final_churn_risk  REAL    NOT NULL,
    final_satisfaction REAL   NOT NULL,
    total_complaints  INTEGER NOT NULL,
    total_fee_burden  REAL    NOT NULL,
    had_retention_offer INTEGER NOT NULL DEFAULT 0,
    primary_churn_driver TEXT NOT NULL,
    PRIMARY KEY (run_id, cohort_id)
);
CREATE INDEX IF NOT EXISTS idx_churn_cohort_segment
    ON churn_cohort (run_id, segment, tick_churned);

CREATE TABLE IF NOT EXISTS churn_aggregate (
    run_id              TEXT    NOT NULL,
    tick                INTEGER NOT NULL,
    segment             TEXT    NOT NULL,
    -- Counts
    active_customers    INTEGER NOT NULL DEFAULT 0,
    churned_this_period INTEGER NOT NULL DEFAULT 0,
    high_risk_count     INTEGER NOT NULL DEFAULT 0,
    -- Rates
    churn_rate          REAL    NOT NULL DEFAULT 0.0,
    avg_churn_risk      REAL    NOT NULL DEFAULT 0.0,
    -- Drivers
    fee_driven_churn    INTEGER NOT NULL DEFAULT 0,
    service_driven_churn INTEGER NOT NULL DEFAULT 0,
    life_event_churn    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (run_id, tick, segment)
);
