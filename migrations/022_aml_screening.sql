-- Phase 3.5 Week 4: AML Screening & Sanctions Monitoring
-- Migration 022: OFAC screening, PEP detection, risk rating, watchlist matching

-- ── OFAC Sanctions Watchlist ──────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS ofac_watchlist (
    entity_id           TEXT PRIMARY KEY,
    entity_type         TEXT NOT NULL,
    -- 'individual', 'business', 'vessel', 'aircraft'
    full_name           TEXT NOT NULL,
    aliases             TEXT,
    -- JSON array of alternative names
    program             TEXT NOT NULL,
    -- 'SDN', 'non-SDN', 'sectoral', 'foreign_sanctions_evaders'
    country_codes       TEXT,
    -- JSON array of ISO country codes
    address_fragments   TEXT,
    -- JSON array for fuzzy matching
    id_numbers          TEXT,
    -- JSON array of passport/national IDs
    date_of_birth       TEXT,
    -- YYYY-MM-DD or partial
    risk_level          TEXT NOT NULL DEFAULT 'high',
    -- 'critical', 'high', 'medium'
    effective_date      TEXT NOT NULL,
    remarks             TEXT
);

CREATE INDEX IF NOT EXISTS idx_ofac_name
    ON ofac_watchlist(full_name);
CREATE INDEX IF NOT EXISTS idx_ofac_program
    ON ofac_watchlist(program);

-- Seed sample OFAC entries for simulation
INSERT OR IGNORE INTO ofac_watchlist (entity_id, entity_type, full_name, aliases, program, country_codes, risk_level, effective_date)
VALUES
    ('OFAC-SDN-001', 'individual', 'Nikolai Petrov', '["Nick Peterson", "N. Petrov"]', 'SDN', '["RU", "BY"]', 'critical', '2020-01-15'),
    ('OFAC-SDN-002', 'individual', 'Ali Hassan Al-Mansoori', '["Ali Al-Mansour"]', 'SDN', '["SY", "LB"]', 'critical', '2019-08-22'),
    ('OFAC-SDN-003', 'business', 'Global Trade Holdings Ltd', '["GTH Ltd", "Global Trading"]', 'SDN', '["VG", "CY"]', 'critical', '2021-03-10'),
    ('OFAC-SDN-004', 'individual', 'Maria Oliveira Santos', '["M. Santos"]', 'non-SDN', '["VE"]', 'high', '2022-06-01'),
    ('OFAC-SDN-005', 'business', 'Northern Energy Consortium', '["NEC", "North Energy"]', 'sectoral', '["RU"]', 'high', '2018-12-05');

-- ── PEP (Politically Exposed Persons) Registry ────────────────────────────

CREATE TABLE IF NOT EXISTS pep_registry (
    pep_id              TEXT PRIMARY KEY,
    full_name           TEXT NOT NULL,
    country_code        TEXT NOT NULL,
    -- ISO 3166-1 alpha-2
    position            TEXT NOT NULL,
    -- 'head_of_state', 'minister', 'legislator', 'military_officer', 'judge', 'diplomat', 'soe_executive'
    position_level      TEXT NOT NULL,
    -- 'tier_1_national', 'tier_2_senior', 'tier_3_local'
    organization        TEXT,
    start_date          TEXT,
    -- YYYY-MM-DD or partial
    end_date            TEXT,
    -- NULL if current
    is_current          INTEGER NOT NULL DEFAULT 1,
    family_members      TEXT,
    -- JSON array of known associates/family
    risk_multiplier     REAL NOT NULL DEFAULT 1.0
);

CREATE INDEX IF NOT EXISTS idx_pep_name
    ON pep_registry(full_name);
CREATE INDEX IF NOT EXISTS idx_pep_country
    ON pep_registry(country_code);

-- Seed sample PEPs
INSERT OR IGNORE INTO pep_registry (pep_id, full_name, country_code, position, position_level, organization, start_date, is_current, risk_multiplier)
VALUES
    ('PEP-001', 'Alexander Volkov', 'RU', 'minister', 'tier_1_national', 'Ministry of Energy', '2015-01-01', 1, 2.5),
    ('PEP-002', 'Ahmed bin Khalid', 'AE', 'soe_executive', 'tier_2_senior', 'National Oil Company', '2018-06-15', 1, 1.8),
    ('PEP-003', 'Sofia Martinez', 'MX', 'legislator', 'tier_2_senior', 'Senate', '2020-01-01', 1, 1.5),
    ('PEP-004', 'Chen Wei', 'CN', 'soe_executive', 'tier_1_national', 'State Construction Corp', '2016-03-01', 1, 2.2),
    ('PEP-005', 'Jean-Pierre Dubois', 'CD', 'head_of_state', 'tier_1_national', 'Office of the President', '2019-01-01', 1, 3.0);

-- ── High-Risk Jurisdiction Configuration ──────────────────────────────────

CREATE TABLE IF NOT EXISTS high_risk_jurisdictions (
    country_code        TEXT PRIMARY KEY,
    -- ISO 3166-1 alpha-2
    country_name        TEXT NOT NULL,
    risk_category       TEXT NOT NULL,
    -- 'fatf_blacklist', 'fatf_greylist', 'sanctions', 'high_corruption', 'tax_haven', 'sec_168j'
    risk_level          TEXT NOT NULL,
    -- 'critical', 'high', 'elevated', 'medium'
    fatf_status         TEXT,
    -- 'blacklist', 'greylist', 'compliant', NULL
    cpi_score           INTEGER,
    -- Corruption Perceptions Index (0-100, lower = more corrupt)
    enhanced_dd_required INTEGER NOT NULL DEFAULT 1,
    -- 1 = require enhanced due diligence
    effective_date      TEXT NOT NULL,
    notes               TEXT
);

CREATE INDEX IF NOT EXISTS idx_hrj_risk_level
    ON high_risk_jurisdictions(risk_level);

-- Seed high-risk jurisdictions
INSERT OR IGNORE INTO high_risk_jurisdictions (country_code, country_name, risk_category, risk_level, fatf_status, cpi_score, enhanced_dd_required, effective_date)
VALUES
    ('KP', 'North Korea', 'fatf_blacklist', 'critical', 'blacklist', 17, 1, '2010-01-01'),
    ('IR', 'Iran', 'fatf_blacklist', 'critical', 'blacklist', 25, 1, '2012-01-01'),
    ('MM', 'Myanmar', 'fatf_greylist', 'high', 'greylist', 28, 1, '2020-06-01'),
    ('SY', 'Syria', 'sanctions', 'critical', NULL, 13, 1, '2011-01-01'),
    ('VE', 'Venezuela', 'sanctions', 'high', NULL, 14, 1, '2019-01-01'),
    ('AF', 'Afghanistan', 'fatf_greylist', 'high', 'greylist', 16, 1, '2021-08-01'),
    ('BY', 'Belarus', 'sanctions', 'high', NULL, 35, 1, '2020-08-01'),
    ('RU', 'Russia', 'sanctions', 'high', NULL, 28, 1, '2022-02-24'),
    ('PA', 'Panama', 'tax_haven', 'elevated', NULL, 36, 1, '2016-04-01'),
    ('VG', 'British Virgin Islands', 'tax_haven', 'elevated', NULL, NULL, 1, '2015-01-01'),
    ('CY', 'Cyprus', 'tax_haven', 'elevated', NULL, 52, 1, '2018-01-01');

-- ── AML Screening Results ──────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS aml_screening_result (
    screening_id        TEXT NOT NULL,
    run_id              TEXT NOT NULL,
    customer_id         TEXT NOT NULL,
    screening_tick      INTEGER NOT NULL,
    screening_type      TEXT NOT NULL,
    -- 'ofac_sanctions', 'pep_match', 'jurisdiction_risk', 'adverse_media'
    match_type          TEXT NOT NULL,
    -- 'exact_match', 'fuzzy_match', 'alias_match', 'none'
    match_score         REAL NOT NULL,
    -- 0.0 to 1.0
    matched_entity_id   TEXT,
    -- Reference to watchlist/PEP entry
    details             TEXT,
    -- JSON with match details
    status              TEXT NOT NULL DEFAULT 'pending_review',
    -- 'pending_review', 'false_positive', 'confirmed', 'escalated'
    risk_impact         REAL DEFAULT 0.0,
    -- Impact on customer risk score
    reviewed_tick       INTEGER,
    review_notes        TEXT,

    PRIMARY KEY (run_id, screening_id),
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);

CREATE INDEX IF NOT EXISTS idx_aml_screening_customer
    ON aml_screening_result(run_id, customer_id, screening_tick DESC);
CREATE INDEX IF NOT EXISTS idx_aml_screening_status
    ON aml_screening_result(run_id, status, match_type);
CREATE INDEX IF NOT EXISTS idx_aml_screening_type
    ON aml_screening_result(run_id, screening_type, screening_tick DESC);

-- ── Customer AML Risk Rating ───────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS customer_aml_risk (
    customer_id         TEXT NOT NULL,
    run_id              TEXT NOT NULL,
    tick                INTEGER NOT NULL,
    overall_risk_rating TEXT NOT NULL,
    -- 'low', 'medium', 'high', 'critical'
    risk_score          REAL NOT NULL,
    -- 0.0 to 1.0

    -- Risk components
    sanctions_risk      REAL DEFAULT 0.0,
    pep_risk            REAL DEFAULT 0.0,
    jurisdiction_risk   REAL DEFAULT 0.0,
    transaction_risk    REAL DEFAULT 0.0,
    behavioral_risk     REAL DEFAULT 0.0,

    -- Metadata
    last_screening_tick INTEGER NOT NULL,
    requires_edd        INTEGER NOT NULL DEFAULT 0,
    -- Enhanced Due Diligence flag
    next_review_tick    INTEGER,

    PRIMARY KEY (run_id, customer_id, tick),
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);

CREATE INDEX IF NOT EXISTS idx_customer_aml_risk_rating
    ON customer_aml_risk(run_id, overall_risk_rating, tick DESC);
CREATE INDEX IF NOT EXISTS idx_customer_aml_risk_score
    ON customer_aml_risk(run_id, risk_score DESC, tick DESC);

-- ── AML Alerts ─────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS aml_alert (
    alert_id            TEXT NOT NULL,
    run_id              TEXT NOT NULL,
    customer_id         TEXT NOT NULL,
    tick                INTEGER NOT NULL,
    alert_type          TEXT NOT NULL,
    -- 'sanctions_hit', 'pep_identified', 'high_risk_jurisdiction', 'risk_rating_elevated'
    severity            TEXT NOT NULL,
    -- 'low', 'medium', 'high', 'critical'
    description         TEXT NOT NULL,
    details             TEXT,
    -- JSON
    status              TEXT NOT NULL DEFAULT 'open',
    -- 'open', 'investigating', 'resolved', 'sar_filed'
    assigned_to         TEXT,
    resolved_tick       INTEGER,
    resolution_notes    TEXT,

    PRIMARY KEY (run_id, alert_id),
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);

CREATE INDEX IF NOT EXISTS idx_aml_alert_customer
    ON aml_alert(run_id, customer_id, tick DESC);
CREATE INDEX IF NOT EXISTS idx_aml_alert_status
    ON aml_alert(run_id, status, severity, tick DESC);
CREATE INDEX IF NOT EXISTS idx_aml_alert_type
    ON aml_alert(run_id, alert_type, tick DESC);

-- ── AML Metrics (Weekly) ───────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS aml_metrics (
    run_id              TEXT NOT NULL,
    tick                INTEGER NOT NULL,
    screenings_performed INTEGER NOT NULL DEFAULT 0,
    sanctions_hits      INTEGER NOT NULL DEFAULT 0,
    pep_matches         INTEGER NOT NULL DEFAULT 0,
    high_risk_customers INTEGER NOT NULL DEFAULT 0,
    alerts_generated    INTEGER NOT NULL DEFAULT 0,
    false_positive_rate REAL DEFAULT 0.0,

    PRIMARY KEY (run_id, tick),
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
