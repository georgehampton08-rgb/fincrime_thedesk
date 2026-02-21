-- Phase 3.4: Card Dispute & Chargeback System
-- Migration 020: Dispute lifecycle, friendly fraud detection, chargeback tracking

-- ── Card Disputes ────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS card_dispute (
    dispute_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    authorization_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    -- Dispute details
    tick_filed INTEGER NOT NULL,
    tick_resolved INTEGER,
    amount REAL NOT NULL,
    merchant_name TEXT NOT NULL,
    merchant_category TEXT NOT NULL,
    reason TEXT NOT NULL,
    -- Status & decision
    status TEXT NOT NULL DEFAULT 'open',
    -- 'open', 'investigating', 'awaiting_merchant', 'under_review',
    -- 'resolved_accepted', 'resolved_rejected', 'resolved_arbitration', 'closed'
    outcome TEXT,
    -- 'accepted', 'rejected', 'arbitration'
    decision_tick INTEGER,
    -- Financial tracking
    provisional_credit_issued INTEGER NOT NULL DEFAULT 0,
    provisional_credit_amount REAL NOT NULL DEFAULT 0.0,
    chargeback_issued INTEGER NOT NULL DEFAULT 0,
    -- Fraud detection
    friendly_fraud_score REAL NOT NULL DEFAULT 0.0,
    PRIMARY KEY (run_id, dispute_id),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_dispute_run_status
    ON card_dispute(run_id, status);
CREATE INDEX IF NOT EXISTS idx_dispute_run_tick
    ON card_dispute(run_id, tick_filed);
CREATE INDEX IF NOT EXISTS idx_dispute_auth
    ON card_dispute(run_id, authorization_id);
CREATE INDEX IF NOT EXISTS idx_dispute_account
    ON card_dispute(run_id, account_id);

-- ── Dispute Timeline ──────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS dispute_timeline (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    dispute_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    from_status TEXT NOT NULL,
    to_status TEXT NOT NULL,
    notes TEXT,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_timeline_dispute
    ON dispute_timeline(run_id, dispute_id);

-- ── Chargeback Metrics ────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS chargeback_metrics (
    run_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    -- Volume metrics
    disputes_filed_7d INTEGER NOT NULL DEFAULT 0,
    chargebacks_issued_7d INTEGER NOT NULL DEFAULT 0,
    disputes_resolved_7d INTEGER NOT NULL DEFAULT 0,
    customer_wins_7d INTEGER NOT NULL DEFAULT 0,
    merchant_wins_7d INTEGER NOT NULL DEFAULT 0,
    -- Financial metrics
    total_disputed_amount_7d REAL NOT NULL DEFAULT 0.0,
    total_chargeback_amount_7d REAL NOT NULL DEFAULT 0.0,
    -- Performance metrics
    win_rate_7d REAL NOT NULL DEFAULT 0.0,
    friendly_fraud_detected_7d INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (run_id, tick),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_chargeback_metrics_tick
    ON chargeback_metrics(run_id, tick DESC);

-- ── Dispute Decision Config ───────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS dispute_decision_config (
    reason TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    win_probability REAL NOT NULL,
    -- Likelihood bank wins dispute (0.0 to 1.0)
    investigation_duration_ticks INTEGER NOT NULL,
    -- Days to investigate before progressing
    merchant_category_risk TEXT NOT NULL
    -- 'low', 'medium', 'high'
);

-- Seed dispute reason configurations (12 common reasons)
INSERT OR IGNORE INTO dispute_decision_config
    (reason, label, win_probability, investigation_duration_ticks, merchant_category_risk)
VALUES
    ('unauthorized_charge', 'Unauthorized Charge', 0.75, 14, 'high'),
    ('duplicate_charge', 'Duplicate Charge', 0.85, 7, 'medium'),
    ('service_not_rendered', 'Service Not Rendered', 0.60, 21, 'medium'),
    ('defective_product', 'Defective Product', 0.55, 21, 'medium'),
    ('not_as_described', 'Not as Described', 0.50, 14, 'medium'),
    ('cancelled_subscription', 'Cancelled Subscription', 0.65, 14, 'high'),
    ('incorrect_amount', 'Incorrect Amount', 0.80, 7, 'low'),
    ('atm_error', 'ATM Error', 0.70, 7, 'low'),
    ('merchant_fraud', 'Merchant Fraud', 0.65, 21, 'high'),
    ('processing_error', 'Processing Error', 0.75, 7, 'low'),
    ('credit_not_received', 'Credit Not Received', 0.55, 14, 'medium'),
    ('card_stolen', 'Card Stolen', 0.80, 14, 'high');
