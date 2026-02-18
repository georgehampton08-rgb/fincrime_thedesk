-- FinCrime: The Desk — Migration 010: Segment Profitability

CREATE TABLE IF NOT EXISTS segment_pnl (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id                   TEXT    NOT NULL,
    tick                     INTEGER NOT NULL,
    segment                  TEXT    NOT NULL,
    -- Revenue
    nii                      REAL    NOT NULL DEFAULT 0.0,
    fee_income               REAL    NOT NULL DEFAULT 0.0,
    interchange_income       REAL    NOT NULL DEFAULT 0.0,
    gross_income             REAL    NOT NULL DEFAULT 0.0,
    -- Costs
    acquisition_cost         REAL    NOT NULL DEFAULT 0.0,
    servicing_cost           REAL    NOT NULL DEFAULT 0.0,
    complaint_cost           REAL    NOT NULL DEFAULT 0.0,
    retention_cost           REAL    NOT NULL DEFAULT 0.0,
    churn_replacement_cost   REAL    NOT NULL DEFAULT 0.0,
    allocated_opex           REAL    NOT NULL DEFAULT 0.0,
    total_cost               REAL    NOT NULL DEFAULT 0.0,
    -- Bottom line
    segment_profit           REAL    NOT NULL DEFAULT 0.0,
    customer_margin          REAL    NOT NULL DEFAULT 0.0,
    profit_per_customer      REAL    NOT NULL DEFAULT 0.0,
    -- Context
    active_customers         INTEGER NOT NULL DEFAULT 0,
    avg_balance              REAL    NOT NULL DEFAULT 0.0,
    avg_revenue_per_customer REAL    NOT NULL DEFAULT 0.0,
    avg_cost_per_customer    REAL    NOT NULL DEFAULT 0.0,
    -- Flags
    below_target_margin      INTEGER NOT NULL DEFAULT 0,
    cross_subsidy_recipient  INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_segment_pnl_tick
    ON segment_pnl (run_id, tick DESC, segment);
CREATE INDEX IF NOT EXISTS idx_segment_pnl_profitability
    ON segment_pnl (run_id, segment_profit DESC);

CREATE TABLE IF NOT EXISTS segment_clv (
    run_id                 TEXT    NOT NULL,
    segment                TEXT    NOT NULL,
    tick                   INTEGER NOT NULL,
    -- CLV components
    expected_revenue       REAL    NOT NULL,
    expected_costs         REAL    NOT NULL,
    clv_npv                REAL    NOT NULL,
    -- Assumptions
    projected_tenure_ticks INTEGER NOT NULL,
    projected_churn_rate   REAL    NOT NULL,
    cross_sell_value       REAL    NOT NULL DEFAULT 0.0,
    discount_rate          REAL    NOT NULL,
    PRIMARY KEY (run_id, segment, tick)
);

CREATE TABLE IF NOT EXISTS cost_allocation_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id          TEXT    NOT NULL,
    tick            INTEGER NOT NULL,
    cost_type       TEXT    NOT NULL,
    segment         TEXT    NOT NULL,
    amount          REAL    NOT NULL,
    allocation_basis TEXT   NOT NULL,
    customer_id     TEXT,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_cost_allocation_segment
    ON cost_allocation_log (run_id, tick, segment, cost_type);

CREATE TABLE IF NOT EXISTS cross_subsidy_analysis (
    run_id            TEXT    NOT NULL,
    tick              INTEGER NOT NULL,
    subsidy_provider  TEXT    NOT NULL,
    subsidy_recipient TEXT    NOT NULL,
    subsidy_amount    REAL    NOT NULL,
    PRIMARY KEY (run_id, tick, subsidy_provider, subsidy_recipient)
);
