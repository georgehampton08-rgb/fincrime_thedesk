-- FinCrime: The Desk — Migration 005: Economics & P&L

CREATE TABLE IF NOT EXISTS pnl_snapshot (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    tick                INTEGER NOT NULL,
    period              TEXT    NOT NULL,
    -- Revenue
    nii                 REAL    NOT NULL DEFAULT 0.0,
    fee_income          REAL    NOT NULL DEFAULT 0.0,
    gross_income        REAL    NOT NULL DEFAULT 0.0,
    -- Costs
    credit_loss         REAL    NOT NULL DEFAULT 0.0,
    fraud_loss          REAL    NOT NULL DEFAULT 0.0,
    opex                REAL    NOT NULL DEFAULT 0.0,
    complaint_cost      REAL    NOT NULL DEFAULT 0.0,
    -- Bottom line
    pre_tax_profit      REAL    NOT NULL DEFAULT 0.0,
    -- KPIs
    nim                 REAL    NOT NULL DEFAULT 0.0,
    efficiency_ratio    REAL    NOT NULL DEFAULT 0.0,
    -- Context
    avg_deposits        REAL    NOT NULL DEFAULT 0.0,
    avg_loans           REAL    NOT NULL DEFAULT 0.0,
    customer_count      INTEGER NOT NULL DEFAULT 0,
    active_accounts     INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_pnl_period
    ON pnl_snapshot (run_id, tick DESC);

CREATE TABLE IF NOT EXISTS staffing_config (
    run_id              TEXT    PRIMARY KEY,
    -- Phase 1D: minimal staffing (just a cost driver)
    -- Phase 3: expands to include analyst capacity model
    base_staff_count    INTEGER NOT NULL DEFAULT 20,
    loaded_cost_per_staff REAL  NOT NULL DEFAULT 85000.0,
    overhead_multiplier REAL    NOT NULL DEFAULT 1.8,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
