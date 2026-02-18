-- FinCrime: The Desk — Migration 013: Payment Rails & Card Authorization Lifecycle
-- ── Reference: Payment rail configurations ────────────────────────
CREATE TABLE IF NOT EXISTS payment_rail (
    rail_id TEXT PRIMARY KEY,
    rail_type TEXT NOT NULL,
    -- 'ACH', 'wire', 'RTP', 'card'
    latency_type TEXT NOT NULL,
    -- 'real_time', 'batch', 'delayed'
    settlement_delay_ticks INTEGER NOT NULL,
    fraud_risk_multiplier REAL NOT NULL DEFAULT 1.0,
    operational_risk_base REAL NOT NULL DEFAULT 0.001,
    batch_window_ticks INTEGER,
    -- NULL for real-time rails
    cutoff_time_tick INTEGER -- Daily cut-off for same-day processing
);
-- Seed the 4 payment rails
INSERT
    OR IGNORE INTO payment_rail (
        rail_id,
        rail_type,
        latency_type,
        settlement_delay_ticks,
        fraud_risk_multiplier,
        operational_risk_base,
        batch_window_ticks,
        cutoff_time_tick
    )
VALUES (
        'ACH',
        'ACH',
        'batch',
        1,
        0.5,
        0.001,
        4,
        NULL
    ),
    (
        'wire',
        'wire',
        'real_time',
        0,
        1.5,
        0.002,
        NULL,
        18
    ),
    (
        'RTP',
        'RTP',
        'real_time',
        0,
        2.0,
        0.0015,
        NULL,
        NULL
    ),
    (
        'card',
        'card',
        'batch',
        1,
        1.2,
        0.0012,
        1,
        NULL
    );
-- ── Card authorization lifecycle ──────────────────────────────────
CREATE TABLE IF NOT EXISTS authorization (
    authorization_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    merchant_name TEXT,
    merchant_category TEXT,
    amount REAL NOT NULL,
    tick_authorized INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    -- 'pending', 'captured', 'reversed', 'expired'
    tick_cleared INTEGER,
    cleared_amount REAL,
    -- May differ from authorized amount
    tick_settled INTEGER,
    interchange_fee REAL,
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (account_id) REFERENCES account(account_id)
);
CREATE INDEX IF NOT EXISTS idx_auth_account ON authorization (run_id, account_id, status);
CREATE INDEX IF NOT EXISTS idx_auth_status_tick ON authorization (run_id, status, tick_authorized);
-- ── Payment batch tracking ────────────────────────────────────────
CREATE TABLE IF NOT EXISTS payment_batch (
    batch_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    rail_id TEXT NOT NULL,
    tick_created INTEGER NOT NULL,
    tick_processed INTEGER,
    item_count INTEGER NOT NULL,
    total_amount REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    -- 'pending', 'processing', 'settled', 'failed'
    exception_count INTEGER DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (rail_id) REFERENCES payment_rail(rail_id)
);
CREATE INDEX IF NOT EXISTS idx_batch_run_rail ON payment_batch (run_id, rail_id, tick_created);
-- ── External settlement statements ────────────────────────────────
CREATE TABLE IF NOT EXISTS external_statement (
    statement_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    rail_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    total_debits REAL NOT NULL DEFAULT 0.0,
    total_credits REAL NOT NULL DEFAULT 0.0,
    item_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (rail_id) REFERENCES payment_rail(rail_id),
    UNIQUE(run_id, rail_id, tick)
);
-- ── Add available_balance to account ──────────────────────────────
-- available_balance = balance - sum(pending authorizations)
-- We add the column; existing rows default to 0.0 and are synced on first use.
ALTER TABLE account
ADD COLUMN available_balance REAL NOT NULL DEFAULT 0.0;
-- ── Add payment rail tracking to transactions ─────────────────────
ALTER TABLE transactions
ADD COLUMN payment_rail_id TEXT DEFAULT 'ACH';
ALTER TABLE transactions
ADD COLUMN settlement_status TEXT NOT NULL DEFAULT 'settled';