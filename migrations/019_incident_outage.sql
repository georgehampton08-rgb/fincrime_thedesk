-- Phase 3.3: Incident & Outage Engine
-- Migration 019: system components, incidents, impacts, and metrics.
-- ── System Components ────────────────────────────────────────────────────────
-- One row per infrastructure component in the simulated bank.
CREATE TABLE IF NOT EXISTS system_component (
    component_id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    category TEXT NOT NULL,
    -- 'core' | 'payments' | 'channels' | 'risk' | 'data'
    technology_tier TEXT NOT NULL DEFAULT 'legacy',
    status TEXT NOT NULL DEFAULT 'operational',
    -- 'operational' | 'degraded' | 'down'
    mtbf_days REAL NOT NULL,
    mttr_hours REAL NOT NULL,
    last_incident_tick INTEGER,
    upgrade_in_progress INTEGER NOT NULL DEFAULT 0,
    upgrade_target_tier TEXT,
    upgrade_complete_tick INTEGER
);
-- Seed: 10 legacy-tier components
INSERT
    OR IGNORE INTO system_component (
        component_id,
        label,
        category,
        mtbf_days,
        mttr_hours
    )
VALUES (
        'core_banking',
        'Core Banking System',
        'core',
        60.0,
        8.0
    ),
    (
        'payment_hub',
        'Payment Hub',
        'payments',
        45.0,
        4.0
    ),
    (
        'card_processor',
        'Card Processor',
        'payments',
        50.0,
        2.0
    ),
    (
        'data_warehouse',
        'Data Warehouse',
        'data',
        90.0,
        12.0
    ),
    (
        'fraud_engine',
        'Fraud Detection Engine',
        'risk',
        40.0,
        6.0
    ),
    (
        'aml_screening',
        'AML Screening',
        'risk',
        70.0,
        4.0
    ),
    (
        'online_banking',
        'Online Banking Portal',
        'channels',
        30.0,
        2.0
    ),
    (
        'mobile_banking',
        'Mobile Banking App',
        'channels',
        35.0,
        3.0
    ),
    (
        'customer_service',
        'Customer Service Platform',
        'channels',
        80.0,
        1.0
    ),
    (
        'network',
        'Network Infrastructure',
        'core',
        120.0,
        6.0
    );
-- ── Incidents ────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS incident (
    incident_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    component_id TEXT NOT NULL,
    tick_created INTEGER NOT NULL,
    tick_resolved INTEGER,
    severity TEXT NOT NULL,
    -- 'P0' | 'P1' | 'P2' | 'P3'
    status TEXT NOT NULL DEFAULT 'open',
    -- 'open' | 'resolved' | 'sla_breached'
    description TEXT NOT NULL,
    sla_deadline_tick INTEGER NOT NULL,
    sla_breached INTEGER NOT NULL DEFAULT 0,
    estimated_revenue_impact REAL NOT NULL DEFAULT 0.0,
    PRIMARY KEY (run_id, incident_id)
);
CREATE INDEX IF NOT EXISTS idx_incident_run_status ON incident(run_id, status);
CREATE INDEX IF NOT EXISTS idx_incident_run_component ON incident(run_id, component_id);
-- ── Incident Impact ──────────────────────────────────────────────────────────
-- Per-tick cascading impact records.  Downstream subsystems query this table
-- to adjust their behaviour while an incident is active.
CREATE TABLE IF NOT EXISTS incident_impact (
    impact_id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    incident_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    impact_type TEXT NOT NULL,
    -- 'transaction_failure_rate' | 'complaint_multiplier' | 'recon_exception_multiplier' | 'fraud_detection_disabled'
    affected_component TEXT NOT NULL,
    impact_value REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX IF NOT EXISTS idx_impact_run_tick ON incident_impact(run_id, tick, impact_type);
-- ── System Metrics ───────────────────────────────────────────────────────────
-- Weekly operational metrics per component.
CREATE TABLE IF NOT EXISTS system_metrics (
    run_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    component_id TEXT NOT NULL,
    uptime_pct REAL NOT NULL,
    incident_count INTEGER NOT NULL,
    avg_mttr_hours REAL NOT NULL DEFAULT 0.0,
    sla_breach_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (run_id, tick, component_id)
);