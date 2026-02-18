-- FinCrime: The Desk — Migration 006: Product Pricing

CREATE TABLE IF NOT EXISTS product_state (
    run_id            TEXT    NOT NULL,
    product_id        TEXT    NOT NULL,
    -- Current active fees (may differ from catalog defaults)
    monthly_fee       REAL    NOT NULL,
    overdraft_fee     REAL    NOT NULL,
    nsf_fee           REAL    NOT NULL,
    atm_fee           REAL    NOT NULL,
    wire_fee          REAL    NOT NULL,
    interest_rate     REAL    NOT NULL,
    -- Tracking
    last_modified_tick INTEGER,
    PRIMARY KEY (run_id, product_id),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

CREATE TABLE IF NOT EXISTS fee_change_log (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id            TEXT    NOT NULL,
    tick              INTEGER NOT NULL,
    product_id        TEXT    NOT NULL,
    fee_type          TEXT    NOT NULL,
    old_value         REAL    NOT NULL,
    new_value         REAL    NOT NULL,
    player_initiated  INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_fee_change_log_product
    ON fee_change_log (run_id, product_id, tick DESC);

CREATE TABLE IF NOT EXISTS regulatory_score (
    run_id            TEXT    PRIMARY KEY,
    udaap_risk_score  REAL    NOT NULL DEFAULT 0.0,
    -- Phase 2.1: UDAAP is the only regulatory risk modeled
    -- Phase 3: expands to full regulatory relationship capital
    last_updated_tick INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
