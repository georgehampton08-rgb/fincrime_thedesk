-- FinCrime: The Desk — Migration 007: Offers & Incentives

CREATE TABLE IF NOT EXISTS customer_offer (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    customer_id         TEXT    NOT NULL,
    offer_id            TEXT    NOT NULL,
    tick_offered        INTEGER NOT NULL,
    tick_accepted       INTEGER,
    tick_completed      INTEGER,
    tick_paid           INTEGER,
    status              TEXT    NOT NULL DEFAULT 'offered',
    -- Status: offered | accepted | in_progress | completed | paid | expired | withdrawn
    bonus_amount        REAL    NOT NULL DEFAULT 0.0,
    bonus_paid          REAL    NOT NULL DEFAULT 0.0,
    requirements_met    INTEGER NOT NULL DEFAULT 0,
    -- Requirements tracking
    cumulative_dd       REAL    NOT NULL DEFAULT 0.0,
    min_balance_days    INTEGER NOT NULL DEFAULT 0,
    ticks_in_offer      INTEGER NOT NULL DEFAULT 0,
    -- Fraud risk flags
    bonus_seeker_flag   INTEGER NOT NULL DEFAULT 0,
    velocity_flag       INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_customer_offer_status
    ON customer_offer (run_id, status, tick_offered);
CREATE INDEX IF NOT EXISTS idx_customer_offer_customer
    ON customer_offer (run_id, customer_id, status);

CREATE TABLE IF NOT EXISTS offer_performance (
    run_id                   TEXT    NOT NULL,
    offer_id                 TEXT    NOT NULL,
    tick                     INTEGER NOT NULL,
    offered_count            INTEGER NOT NULL DEFAULT 0,
    accepted_count           INTEGER NOT NULL DEFAULT 0,
    completed_count          INTEGER NOT NULL DEFAULT 0,
    expired_count            INTEGER NOT NULL DEFAULT 0,
    total_bonus_paid         REAL    NOT NULL DEFAULT 0.0,
    avg_bonus_per_completion REAL    NOT NULL DEFAULT 0.0,
    bonus_seeker_count       INTEGER NOT NULL DEFAULT 0,
    velocity_flag_count      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (run_id, offer_id, tick)
);

CREATE TABLE IF NOT EXISTS offer_config_state (
    run_id        TEXT    NOT NULL,
    offer_id      TEXT    NOT NULL,
    active        INTEGER NOT NULL DEFAULT 1,
    start_tick    INTEGER NOT NULL DEFAULT 0,
    end_tick      INTEGER,
    modified_tick INTEGER,
    PRIMARY KEY (run_id, offer_id)
);
