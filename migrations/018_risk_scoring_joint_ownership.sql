-- Migration 018: Risk scoring, authorized signers, joint ownership, customer relationships
-- Phase 3.5-prep Tier 4
-- ═══════════════════════════════════════════════════════════════════════
-- Customer risk scoring (BSA/AML composite risk)
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS customer_risk_score (
    customer_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    composite_risk TEXT NOT NULL,
    -- low | medium | high | critical
    identity_risk_score REAL NOT NULL,
    -- 0.0-1.0
    geographic_risk_score REAL NOT NULL,
    product_risk_score REAL NOT NULL,
    behavior_risk_score REAL NOT NULL,
    sanctions_risk_score REAL NOT NULL,
    edd_required INTEGER DEFAULT 0,
    -- enhanced due diligence needed
    edd_last_review_tick INTEGER,
    risk_override TEXT,
    -- escalated | de-escalated
    risk_override_reason TEXT,
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);
-- ═══════════════════════════════════════════════════════════════════════
-- Account authorized signers (beyond primary owner)
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS authorized_signer (
    signer_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    signer_customer_id TEXT NOT NULL,
    signer_role TEXT NOT NULL,
    -- owner | co-owner | poa | authorized_signer
    authority_level TEXT NOT NULL,
    -- full | limited | view_only
    added_tick INTEGER NOT NULL,
    removed_tick INTEGER,
    is_active INTEGER DEFAULT 1,
    FOREIGN KEY (account_id) REFERENCES account(account_id)
);
-- ═══════════════════════════════════════════════════════════════════════
-- Joint ownership (multi-party accounts)
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS joint_ownership (
    ownership_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    owner_customer_id TEXT NOT NULL,
    ownership_percentage REAL NOT NULL,
    ownership_type TEXT NOT NULL,
    -- jtros | tic | community_property
    survivorship_rights INTEGER DEFAULT 1,
    FOREIGN KEY (account_id) REFERENCES account(account_id)
);
-- ═══════════════════════════════════════════════════════════════════════
-- Customer relationships (graph links)
-- ═══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS customer_relationship (
    relationship_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    customer_id_a TEXT NOT NULL,
    customer_id_b TEXT NOT NULL,
    relationship_type TEXT NOT NULL,
    -- spouse | parent_child | employer | beneficial_owner
    strength REAL NOT NULL,
    -- 0.0-1.0
    detected_tick INTEGER NOT NULL,
    detection_method TEXT NOT NULL,
    -- declared | inferred_address | inferred_phone | inferred_txn
    is_suspicious INTEGER DEFAULT 0,
    FOREIGN KEY (customer_id_a) REFERENCES customer(customer_id),
    FOREIGN KEY (customer_id_b) REFERENCES customer(customer_id)
);