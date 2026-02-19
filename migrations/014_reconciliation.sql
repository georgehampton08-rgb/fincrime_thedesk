-- FinCrime: The Desk — Migration 014: Reconciliation & Exception Management
-- ── Ledger entry: denormalized settled-transaction log ───────────────
-- PaymentHubSubsystem writes one row per settled item.
-- ReconciliationSubsystem sums this for internal-vs-external comparison.
CREATE TABLE IF NOT EXISTS ledger_entry (
    entry_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    rail_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    -- tick the item settled
    amount REAL NOT NULL,
    -- absolute value of settled amount
    direction TEXT NOT NULL,
    -- 'debit' or 'credit'
    source_txn_id TEXT,
    -- reference to transactions.txn_id
    source_auth_id TEXT,
    -- reference to authorization.authorization_id (card only)
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (rail_id) REFERENCES payment_rail(rail_id)
);
CREATE INDEX IF NOT EXISTS idx_ledger_run_rail_tick ON ledger_entry (run_id, rail_id, tick);
-- ── Reconciliation exceptions ─────────────────────────────────────────
CREATE TABLE IF NOT EXISTS recon_exception (
    exception_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    rail_id TEXT NOT NULL,
    tick_detected INTEGER NOT NULL,
    tick_resolved INTEGER,
    status TEXT NOT NULL DEFAULT 'open',
    -- 'open', 'investigating', 'resolved', 'written_off'
    delta_amount REAL NOT NULL,
    internal_total REAL NOT NULL,
    external_total REAL NOT NULL,
    item_count_delta INTEGER,
    suspected_cause TEXT,
    -- 'timing', 'duplicate', 'missing_item', 'amount_error', 'unknown'
    assigned_to TEXT,
    -- Phase 3.8: employee_id
    resolution_notes TEXT,
    resolution_type TEXT,
    -- 'auto_clear', 'manual_adjustment', 'write_off'
    write_off_amount REAL NOT NULL DEFAULT 0.0,
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (rail_id) REFERENCES payment_rail(rail_id)
);
CREATE INDEX IF NOT EXISTS idx_recon_exc_run_status ON recon_exception (run_id, status, tick_detected);
CREATE INDEX IF NOT EXISTS idx_recon_exc_rail ON recon_exception (run_id, rail_id, status);
-- ── Recon queue configuration (one row per rail) ──────────────────────
CREATE TABLE IF NOT EXISTS recon_queue_config (
    rail_id TEXT PRIMARY KEY,
    tolerance_amount REAL NOT NULL DEFAULT 0.01,
    auto_clear_threshold REAL NOT NULL DEFAULT 1.00,
    sla_days INTEGER NOT NULL DEFAULT 3,
    escalation_threshold REAL NOT NULL DEFAULT 100.00,
    escalation_age_days INTEGER NOT NULL DEFAULT 7,
    FOREIGN KEY (rail_id) REFERENCES payment_rail(rail_id)
);
-- Seed per-rail config
INSERT
    OR IGNORE INTO recon_queue_config (
        rail_id,
        tolerance_amount,
        auto_clear_threshold,
        sla_days,
        escalation_threshold,
        escalation_age_days
    )
VALUES ('ACH', 0.01, 1.00, 3, 100.00, 7),
    ('wire', 0.01, 0.50, 1, 1000.00, 3),
    ('RTP', 0.01, 0.50, 1, 500.00, 3),
    ('card', 1.00, 5.00, 3, 100.00, 7);
-- ── Weekly reconciliation metrics snapshot ────────────────────────────
CREATE TABLE IF NOT EXISTS recon_metrics (
    run_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    rail_id TEXT NOT NULL,
    total_exceptions INTEGER NOT NULL DEFAULT 0,
    open_exceptions INTEGER NOT NULL DEFAULT 0,
    aged_exceptions_7d INTEGER NOT NULL DEFAULT 0,
    aged_exceptions_14d INTEGER NOT NULL DEFAULT 0,
    aged_exceptions_30d INTEGER NOT NULL DEFAULT 0,
    total_delta_amount REAL NOT NULL DEFAULT 0.0,
    unresolved_amount REAL NOT NULL DEFAULT 0.0,
    write_off_amount REAL NOT NULL DEFAULT 0.0,
    auto_cleared INTEGER NOT NULL DEFAULT 0,
    manually_resolved INTEGER NOT NULL DEFAULT 0,
    written_off INTEGER NOT NULL DEFAULT 0,
    avg_resolution_days REAL,
    sla_compliance_pct REAL,
    PRIMARY KEY (run_id, tick, rail_id),
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (rail_id) REFERENCES payment_rail(rail_id)
);
-- ── Regulatory score components for recon penalties ───────────────────
CREATE TABLE IF NOT EXISTS regulatory_score_component (
    run_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    component TEXT NOT NULL,
    -- 'recon_controls', 'recon_write_offs'
    score_delta REAL NOT NULL,
    findings TEXT,
    PRIMARY KEY (run_id, tick, component)
);
CREATE INDEX IF NOT EXISTS idx_reg_score_comp_run_tick ON regulatory_score_component (run_id, tick);