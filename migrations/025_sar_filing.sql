-- Phase 3.5 Week 6: SAR Filing & Integration
-- Migration 025: Suspicious Activity Reports and regulatory filing

-- ── Suspicious Activity Reports ─────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS suspicious_activity_report (
    sar_id              TEXT NOT NULL,
    run_id              TEXT NOT NULL,
    filing_tick         INTEGER NOT NULL,
    subject_type        TEXT NOT NULL,
    -- 'customer', 'account', 'transaction', 'entity'
    subject_id          TEXT NOT NULL,
    -- customer_id, account_id, transaction_id, or entity_id
    activity_type       TEXT NOT NULL,
    -- 'structuring', 'money_laundering', 'fraud', 'terrorist_financing', 'identity_theft', 'elder_abuse'
    suspicious_amount   REAL NOT NULL,
    narrative           TEXT NOT NULL,
    -- Detailed description of suspicious activity
    filing_deadline     INTEGER NOT NULL,
    -- Must file within 30 days of detection
    filed_on_time       INTEGER NOT NULL DEFAULT 1,
    filing_status       TEXT NOT NULL DEFAULT 'filed',
    -- 'filed', 'late', 'missed'
    regulatory_fine     REAL DEFAULT 0.0,
    -- Fine for late/missed filing
    related_alerts      TEXT,
    -- JSON array of alert IDs that triggered this SAR

    PRIMARY KEY (run_id, sar_id),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_sar_subject
    ON suspicious_activity_report(run_id, subject_id, filing_tick DESC);
CREATE INDEX IF NOT EXISTS idx_sar_filing_status
    ON suspicious_activity_report(run_id, filing_status, filing_tick DESC);
CREATE INDEX IF NOT EXISTS idx_sar_activity_type
    ON suspicious_activity_report(run_id, activity_type, filing_tick DESC);

-- ── SAR Filing Configuration ─────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS sar_filing_rule (
    rule_id             TEXT PRIMARY KEY,
    trigger_type        TEXT NOT NULL,
    -- 'alert_threshold', 'pattern_match', 'manual_review', 'auto_file'
    min_alert_score     REAL,
    activity_types      TEXT NOT NULL,
    -- JSON array of activity types that trigger SAR
    auto_file           INTEGER NOT NULL DEFAULT 0,
    filing_priority     TEXT NOT NULL DEFAULT 'normal',
    -- 'urgent', 'high', 'normal', 'low'
    enabled             INTEGER NOT NULL DEFAULT 1
);

-- Seed SAR filing rules
INSERT OR IGNORE INTO sar_filing_rule (
    rule_id, trigger_type, min_alert_score, activity_types, auto_file, filing_priority, enabled
) VALUES
    -- High-confidence structuring automatically files SAR
    ('SAR_STRUCTURING', 'auto_file', 85.0, '["structuring"]', 1, 'high', 1),

    -- Fraud patterns above 80% confidence
    ('SAR_FRAUD_HIGH', 'alert_threshold', 80.0, '["fraud", "identity_theft", "synthetic_identity"]', 1, 'high', 1),

    -- AML screening hits require SAR
    ('SAR_AML_HIT', 'auto_file', 90.0, '["money_laundering", "terrorist_financing"]', 1, 'urgent', 1),

    -- Elder abuse detected
    ('SAR_ELDER_ABUSE', 'auto_file', 70.0, '["elder_abuse", "elder_financial_exploitation"]', 1, 'urgent', 1),

    -- Multiple related alerts
    ('SAR_MULTIPLE_ALERTS', 'pattern_match', 75.0, '["structuring", "velocity", "fraud"]', 0, 'normal', 1);

-- ── SAR Filing Metrics (Monthly) ────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS sar_filing_metrics (
    run_id              TEXT NOT NULL,
    tick                INTEGER NOT NULL,
    sars_filed          INTEGER NOT NULL DEFAULT 0,
    sars_filed_on_time  INTEGER NOT NULL DEFAULT 0,
    sars_late           INTEGER NOT NULL DEFAULT 0,
    sars_missed         INTEGER NOT NULL DEFAULT 0,
    total_fines         REAL DEFAULT 0.0,
    avg_filing_time     REAL DEFAULT 0.0,
    -- Average days from detection to filing

    PRIMARY KEY (run_id, tick),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);

-- ── Update transaction monitoring metrics to include SAR count ──────────────

-- Note: The sars_filed column already exists in transaction_monitoring_metrics
-- from migration 024, so no ALTER TABLE needed here.
