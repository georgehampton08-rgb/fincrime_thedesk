-- Phase 3.5 Week 5: Transaction Monitoring
-- Migration 024: Transaction monitoring rules, alerts, and CTR/SAR triggers

-- ── Transaction Monitoring Rules ────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS transaction_monitoring_rule (
    rule_id             TEXT PRIMARY KEY,
    rule_name           TEXT NOT NULL,
    rule_type           TEXT NOT NULL,
    -- 'structuring', 'velocity', 'amount_threshold', 'geographic_anomaly', 'rapid_movement'
    threshold_amount    REAL,
    threshold_count     INTEGER,
    lookback_days       INTEGER NOT NULL DEFAULT 30,
    base_alert_score    REAL NOT NULL,
    auto_file_sar       INTEGER NOT NULL DEFAULT 0,
    enabled             INTEGER NOT NULL DEFAULT 1
);

-- Seed standard FinCEN rules
INSERT OR IGNORE INTO transaction_monitoring_rule (
    rule_id, rule_name, rule_type, threshold_amount, threshold_count,
    lookback_days, base_alert_score, auto_file_sar, enabled
) VALUES
    -- CTR: Currency Transaction Report for $10k+
    ('CTR_10K', 'Currency Transaction Report', 'amount_threshold', 10000.0, 1, 1, 100.0, 1, 1),

    -- Structuring: Multiple transactions just under $10k
    ('STRUCT_9K', 'Structuring Detection', 'structuring', 9000.0, 3, 7, 90.0, 1, 1),

    -- High Velocity: Many transactions in short period
    ('VEL_50K_7D', 'High Velocity 7-Day', 'velocity', 50000.0, 10, 7, 75.0, 0, 1),
    ('VEL_100K_30D', 'High Velocity 30-Day', 'velocity', 100000.0, 20, 30, 70.0, 0, 1),

    -- Rapid Movement: Immediate withdrawal after deposit
    ('RAPID_MOVE', 'Rapid Money Movement', 'rapid_movement', 5000.0, 1, 1, 80.0, 0, 1),

    -- Geographic Anomaly: Unexpected locations
    ('GEO_ANOMALY', 'Geographic Anomaly Detection', 'geographic_anomaly', 1000.0, 1, 1, 65.0, 0, 1);

-- ── AML Alerts (extend table from migration 022) ──────────────────────────────

-- Add transaction monitoring specific columns to existing aml_alert table
ALTER TABLE aml_alert ADD COLUMN rule_id TEXT;
ALTER TABLE aml_alert ADD COLUMN alert_score REAL DEFAULT 0.0;
ALTER TABLE aml_alert ADD COLUMN triggered_amount REAL;
ALTER TABLE aml_alert ADD COLUMN transaction_count INTEGER;

-- Create additional indexes for transaction monitoring
CREATE INDEX IF NOT EXISTS idx_aml_alert_score
    ON aml_alert(run_id, status, alert_score DESC);
CREATE INDEX IF NOT EXISTS idx_aml_alert_rule
    ON aml_alert(run_id, rule_id, tick DESC);

-- ── Currency Transaction Reports (CTR) ──────────────────────────────────────

CREATE TABLE IF NOT EXISTS currency_transaction_report (
    ctr_id              TEXT NOT NULL,
    run_id              TEXT NOT NULL,
    customer_id         TEXT NOT NULL,
    account_id          TEXT NOT NULL,
    transaction_id      TEXT NOT NULL,
    filing_tick         INTEGER NOT NULL,
    transaction_amount  REAL NOT NULL,
    transaction_type    TEXT NOT NULL,
    -- 'cash_deposit', 'cash_withdrawal', 'multiple_related_transactions'
    filing_deadline     INTEGER NOT NULL,
    filed_on_time       INTEGER NOT NULL DEFAULT 1,
    auto_filed          INTEGER NOT NULL DEFAULT 1,

    PRIMARY KEY (run_id, ctr_id),
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id),
    FOREIGN KEY (account_id) REFERENCES account(account_id),
    FOREIGN KEY (transaction_id) REFERENCES transactions(transaction_id)
);

CREATE INDEX IF NOT EXISTS idx_ctr_customer
    ON currency_transaction_report(run_id, customer_id, filing_tick DESC);
CREATE INDEX IF NOT EXISTS idx_ctr_filing
    ON currency_transaction_report(run_id, filed_on_time, filing_tick DESC);

-- ── Transaction Monitoring Metrics (Weekly) ─────────────────────────────────

CREATE TABLE IF NOT EXISTS transaction_monitoring_metrics (
    run_id              TEXT NOT NULL,
    tick                INTEGER NOT NULL,
    alerts_generated    INTEGER NOT NULL DEFAULT 0,
    alerts_investigating INTEGER NOT NULL DEFAULT 0,
    alerts_closed       INTEGER NOT  NULL DEFAULT 0,
    ctrs_filed          INTEGER NOT NULL DEFAULT 0,
    sars_filed          INTEGER NOT NULL DEFAULT 0,
    -- SAR count (will be added in Week 6)
    false_positive_rate REAL DEFAULT 0.0,
    avg_investigation_time REAL DEFAULT 0.0,
    -- In ticks

    PRIMARY KEY (run_id, tick),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
