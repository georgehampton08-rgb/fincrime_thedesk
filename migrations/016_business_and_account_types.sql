-- Migration 016: Business entities, account types, marital status, beneficiaries
-- Phase 3.5-prep Tier 2
-- ═══════════════════════════════════════════════════════════════════════
-- Account type reference table (13 account types)
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS account_type_config (
    account_type_id TEXT PRIMARY KEY,
    account_category TEXT NOT NULL,
    -- checking|savings|custody|trust|estate|business
    display_name TEXT NOT NULL,
    requires_ssn INTEGER DEFAULT 1,
    requires_ein INTEGER DEFAULT 0,
    allows_minor INTEGER DEFAULT 0,
    requires_custodian INTEGER DEFAULT 0,
    requires_trustee INTEGER DEFAULT 0,
    min_age INTEGER,
    max_owners INTEGER,
    regulatory_category TEXT NOT NULL,
    -- reg_d|reg_e|fiduciary|commercial
    kyc_level TEXT NOT NULL -- standard|enhanced|fiduciary|cdd_plus
);
INSERT
    OR IGNORE INTO account_type_config
VALUES (
        'checking_individual',
        'checking',
        'Personal Checking',
        1,
        0,
        0,
        0,
        0,
        18,
        1,
        'reg_e',
        'standard'
    ),
    (
        'checking_joint',
        'checking',
        'Joint Checking',
        1,
        0,
        0,
        0,
        0,
        18,
        4,
        'reg_e',
        'standard'
    ),
    (
        'savings_individual',
        'savings',
        'Personal Savings',
        1,
        0,
        0,
        0,
        0,
        18,
        1,
        'reg_d',
        'standard'
    ),
    (
        'savings_minor',
        'savings',
        'Minor Savings',
        1,
        0,
        1,
        1,
        0,
        NULL,
        1,
        'reg_d',
        'standard'
    ),
    (
        'utma',
        'custody',
        'UTMA Custodial',
        1,
        0,
        1,
        1,
        0,
        NULL,
        1,
        'fiduciary',
        'fiduciary'
    ),
    (
        'ugma',
        'custody',
        'UGMA Custodial',
        1,
        0,
        1,
        1,
        0,
        NULL,
        1,
        'fiduciary',
        'fiduciary'
    ),
    (
        'trust_revocable',
        'trust',
        'Revocable Living Trust',
        0,
        0,
        0,
        0,
        1,
        18,
        4,
        'fiduciary',
        'fiduciary'
    ),
    (
        'trust_irrevocable',
        'trust',
        'Irrevocable Trust',
        0,
        1,
        0,
        0,
        1,
        18,
        4,
        'fiduciary',
        'enhanced'
    ),
    (
        'estate',
        'trust',
        'Estate Account',
        0,
        1,
        0,
        0,
        1,
        18,
        2,
        'fiduciary',
        'enhanced'
    ),
    (
        'business_checking',
        'business',
        'Business Checking',
        0,
        1,
        0,
        0,
        0,
        18,
        8,
        'commercial',
        'cdd_plus'
    ),
    (
        'business_savings',
        'business',
        'Business Savings',
        0,
        1,
        0,
        0,
        0,
        18,
        8,
        'commercial',
        'cdd_plus'
    ),
    (
        'payable_on_death',
        'checking',
        'Payable On Death (POD)',
        1,
        0,
        0,
        0,
        0,
        18,
        1,
        'reg_e',
        'standard'
    ),
    (
        'representative_payee',
        'checking',
        'Representative Payee',
        1,
        0,
        0,
        1,
        0,
        NULL,
        1,
        'reg_e',
        'enhanced'
    );
-- ═══════════════════════════════════════════════════════════════════════
-- Business entities
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS business_entity (
    entity_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    legal_name TEXT NOT NULL,
    dba_name TEXT,
    entity_type TEXT NOT NULL,
    -- sole_proprietorship|llc|s_corp|c_corp|partnership|nonprofit
    ein TEXT NOT NULL,
    state_registration TEXT NOT NULL,
    formation_date TEXT NOT NULL,
    ownership_type TEXT NOT NULL,
    -- single|multi|partnership
    owner_count INTEGER NOT NULL DEFAULT 1,
    naics_code TEXT NOT NULL,
    annual_revenue REAL,
    employee_count INTEGER,
    is_cash_intensive INTEGER DEFAULT 0,
    is_high_risk_industry INTEGER DEFAULT 0,
    shell_company_indicators INTEGER DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_business_ein ON business_entity(ein);
-- ═══════════════════════════════════════════════════════════════════════
-- DBA registrations
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS dba_registration (
    dba_id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    dba_name TEXT NOT NULL,
    state_registered TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    is_potentially_deceptive INTEGER DEFAULT 0
);
-- ═══════════════════════════════════════════════════════════════════════
-- Beneficiaries
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS customer_beneficiary (
    beneficiary_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    beneficiary_name TEXT NOT NULL,
    beneficiary_relationship TEXT NOT NULL,
    -- spouse|child|parent|sibling|other
    beneficiary_type TEXT NOT NULL,
    -- primary|contingent
    beneficiary_share REAL NOT NULL,
    is_per_stirpes INTEGER DEFAULT 0,
    trust_for_minor INTEGER DEFAULT 0,
    verified INTEGER DEFAULT 0
);
-- ═══════════════════════════════════════════════════════════════════════
-- Extend account table
-- ═══════════════════════════════════════════════════════════════════════
ALTER TABLE account
ADD COLUMN account_type_category TEXT;
ALTER TABLE account
ADD COLUMN ownership_structure TEXT;
ALTER TABLE account
ADD COLUMN tax_reporting_type TEXT;
ALTER TABLE account
ADD COLUMN primary_tax_id TEXT;
-- ═══════════════════════════════════════════════════════════════════════
-- Extend customer table
-- ═══════════════════════════════════════════════════════════════════════
ALTER TABLE customer
ADD COLUMN marital_status TEXT;
ALTER TABLE customer
ADD COLUMN spouse_customer_id TEXT;
ALTER TABLE customer
ADD COLUMN employment_status TEXT;
ALTER TABLE customer
ADD COLUMN annual_income REAL;
ALTER TABLE customer
ADD COLUMN credit_score INTEGER;
ALTER TABLE customer
ADD COLUMN home_ownership TEXT;
ALTER TABLE customer
ADD COLUMN dependents INTEGER DEFAULT 0;
ALTER TABLE customer
ADD COLUMN military_status TEXT;