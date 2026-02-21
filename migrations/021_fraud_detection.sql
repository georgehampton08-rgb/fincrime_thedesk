-- Phase 3.5 Week 3: Fraud Detection Framework
-- Migration 021: Fraud pattern detection, account risk scoring, alert generation

-- ── Fraud Detection Results ─────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS fraud_pattern (
    pattern_id          TEXT NOT NULL,
    run_id              TEXT NOT NULL,
    pattern_type        TEXT NOT NULL,
    -- 'synthetic_identity', 'bust_out', 'money_mule', 'elder_abuse', 'account_takeover'
    detected_tick       INTEGER NOT NULL,
    confidence_score    REAL NOT NULL,
    -- 0.0 to 1.0
    primary_customer_id TEXT,
    primary_account_id  TEXT,
    involved_accounts   TEXT,
    -- JSON array of account IDs
    fraud_indicators    TEXT NOT NULL,
    -- JSON array of indicator descriptions
    status              TEXT NOT NULL DEFAULT 'open',
    -- 'open', 'investigating', 'confirmed', 'false_positive', 'resolved'
    estimated_loss      REAL DEFAULT 0.0,
    actual_loss         REAL DEFAULT 0.0,
    investigation_notes TEXT,
    PRIMARY KEY (run_id, pattern_id),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_fraud_pattern_type
    ON fraud_pattern(run_id, pattern_type, detected_tick);
CREATE INDEX IF NOT EXISTS idx_fraud_pattern_customer
    ON fraud_pattern(run_id, primary_customer_id);
CREATE INDEX IF NOT EXISTS idx_fraud_pattern_status
    ON fraud_pattern(run_id, status, detected_tick DESC);

-- ── Account Fraud Scores ────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS account_fraud_score (
    account_id          TEXT NOT NULL,
    run_id              TEXT NOT NULL,
    tick                INTEGER NOT NULL,
    fraud_risk          REAL NOT NULL,
    -- 0.0 to 1.0 composite score

    -- Score components
    velocity_component      REAL DEFAULT 0.0,
    amount_component        REAL DEFAULT 0.0,
    pattern_component       REAL DEFAULT 0.0,
    behavioral_component    REAL DEFAULT 0.0,
    identity_component      REAL DEFAULT 0.0,

    -- Metadata
    last_alert_tick     INTEGER,
    consecutive_alerts  INTEGER DEFAULT 0,

    PRIMARY KEY (run_id, account_id, tick),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_account_fraud_score_risk
    ON account_fraud_score(run_id, fraud_risk DESC, tick DESC);

-- ── Fraud Alerts ────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS fraud_alert (
    alert_id            TEXT NOT NULL,
    run_id              TEXT NOT NULL,
    tick                INTEGER NOT NULL,
    alert_type          TEXT NOT NULL,
    -- 'transaction_anomaly', 'velocity_spike', 'account_risk_score', 'pattern_match'
    entity_type         TEXT NOT NULL,
    -- 'customer', 'account', 'transaction'
    entity_id           TEXT NOT NULL,
    fraud_score         REAL NOT NULL,
    severity            TEXT NOT NULL,
    -- 'low', 'medium', 'high', 'critical'
    details             TEXT,
    -- JSON details
    investigation_status TEXT DEFAULT 'open',
    -- 'open', 'investigating', 'resolved', 'false_positive'
    assigned_to         TEXT,
    resolved_tick       INTEGER,

    PRIMARY KEY (run_id, alert_id),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_fraud_alert_entity
    ON fraud_alert(run_id, entity_id, tick DESC);
CREATE INDEX IF NOT EXISTS idx_fraud_alert_status
    ON fraud_alert(run_id, investigation_status, severity, tick DESC);
CREATE INDEX IF NOT EXISTS idx_fraud_alert_type
    ON fraud_alert(run_id, alert_type, tick DESC);

-- ── Fraud Detection Configuration ───────────────────────────────────────────

CREATE TABLE IF NOT EXISTS fraud_detection_config (
    config_key          TEXT PRIMARY KEY,
    config_value        TEXT NOT NULL,
    config_type         TEXT NOT NULL,
    -- 'threshold', 'weight', 'enabled', 'window'
    description         TEXT
);

-- Seed default fraud detection configuration
INSERT OR IGNORE INTO fraud_detection_config (config_key, config_value, config_type, description)
VALUES
    ('enabled', 'true', 'enabled', 'Fraud detection subsystem enabled'),
    ('velocity_threshold', '10', 'threshold', 'Max transactions per day before velocity alert'),
    ('amount_zscore_threshold', '3.0', 'threshold', 'Z-score threshold for amount anomaly'),
    ('new_merchant_risk_weight', '0.20', 'weight', 'Risk weight for new merchant transactions'),
    ('account_risk_threshold', '0.60', 'threshold', 'Account risk score threshold for alert'),
    ('synthetic_identity_threshold', '0.60', 'threshold', 'Synthetic identity confidence threshold'),
    ('bust_out_amount_threshold', '5000.0', 'threshold', 'Bust-out pattern amount threshold'),
    ('money_mule_velocity_days', '7', 'window', 'Days to check for money mule velocity'),
    ('elder_abuse_age_threshold', '65', 'threshold', 'Age threshold for elder abuse detection'),
    ('velocity_window_days', '7', 'window', 'Rolling window for velocity calculations'),
    ('metrics_interval_ticks', '7', 'window', 'Ticks between fraud metrics computation');
