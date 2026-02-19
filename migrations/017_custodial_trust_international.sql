-- Migration 017: Custodial accounts (UTMA/UGMA), trust accounts, international customers
-- Phase 3.5-prep Tier 3
-- ═══════════════════════════════════════════════════════════════════════
-- Custodial UTMA/UGMA accounts
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS custodial_account (
    account_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    account_type TEXT NOT NULL,
    -- utma | ugma
    minor_customer_id TEXT NOT NULL,
    minor_dob TEXT NOT NULL,
    age_of_majority INTEGER DEFAULT 18,
    termination_age INTEGER DEFAULT 21,
    custodian_customer_id TEXT NOT NULL,
    custodian_relationship TEXT NOT NULL,
    -- parent | grandparent | guardian
    tax_reporting_ssn TEXT NOT NULL,
    state_governed TEXT NOT NULL
);
-- ═══════════════════════════════════════════════════════════════════════
-- Trust accounts
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS trust_account (
    account_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    trust_type TEXT NOT NULL,
    -- revocable | irrevocable | testamentary | special_needs
    trust_name TEXT NOT NULL,
    trust_ein TEXT,
    grantor_customer_id TEXT,
    trustee_customer_id TEXT NOT NULL,
    trustee_type TEXT NOT NULL,
    -- individual | corporate | co-trustee
    beneficiary_count INTEGER NOT NULL,
    revocable INTEGER NOT NULL,
    tax_reporting_id TEXT NOT NULL,
    tax_treatment TEXT NOT NULL,
    -- grantor | non-grantor
    spendthrift_clause INTEGER DEFAULT 0,
    special_needs_trust INTEGER DEFAULT 0
);
CREATE TABLE IF NOT EXISTS trust_beneficiary (
    beneficiary_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    beneficiary_customer_id TEXT,
    beneficiary_name TEXT NOT NULL,
    beneficiary_type TEXT NOT NULL,
    -- primary | contingent | remainder
    beneficiary_share REAL NOT NULL,
    conditions TEXT
);
-- ═══════════════════════════════════════════════════════════════════════
-- International customers
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS customer_international (
    customer_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    citizenship_country TEXT NOT NULL,
    residency_country TEXT NOT NULL,
    is_us_person INTEGER NOT NULL,
    visa_status TEXT,
    foreign_tin TEXT,
    ofac_check_status TEXT NOT NULL DEFAULT 'clear',
    sanctions_risk TEXT NOT NULL DEFAULT 'low',
    pep_status INTEGER DEFAULT 0,
    source_of_funds TEXT,
    kyc_renewal_date TEXT NOT NULL
);