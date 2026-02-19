-- FinCrime: The Desk — Migration 015: Customer Identity, Address, Phone
-- Phase 3.5-prep: Full identity model for KYC, AML, and fraud detection
-- ─────────────────────────────────────────────────────────────────────────────
-- Reference table: US States, DC, and territories
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS state_config (
    state_code TEXT PRIMARY KEY,
    -- 2-letter abbreviation
    state_name TEXT NOT NULL,
    territory_type TEXT NOT NULL,
    -- 'state' | 'district' | 'territory'
    community_property INTEGER NOT NULL DEFAULT 0,
    utma_termination_age INTEGER NOT NULL DEFAULT 21,
    population INTEGER NOT NULL,
    median_income REAL NOT NULL,
    cost_of_living_index REAL NOT NULL DEFAULT 1.0,
    identity_theft_index REAL NOT NULL DEFAULT 1.0 -- relative fraud risk vs national avg
);
INSERT
    OR IGNORE INTO state_config (
        state_code,
        state_name,
        territory_type,
        community_property,
        utma_termination_age,
        population,
        median_income,
        cost_of_living_index,
        identity_theft_index
    )
VALUES -- 50 States
    (
        'AL',
        'Alabama',
        'state',
        0,
        21,
        5024279,
        52035,
        0.89,
        0.95
    ),
    (
        'AK',
        'Alaska',
        'state',
        0,
        18,
        733391,
        77765,
        1.28,
        0.78
    ),
    (
        'AZ',
        'Arizona',
        'state',
        1,
        21,
        7151502,
        61529,
        1.02,
        1.15
    ),
    (
        'AR',
        'Arkansas',
        'state',
        0,
        21,
        3011524,
        49475,
        0.86,
        0.88
    ),
    (
        'CA',
        'California',
        'state',
        1,
        25,
        39538223,
        78672,
        1.42,
        1.35
    ),
    (
        'CO',
        'Colorado',
        'state',
        0,
        21,
        5773714,
        72331,
        1.10,
        1.12
    ),
    (
        'CT',
        'Connecticut',
        'state',
        0,
        21,
        3605944,
        79855,
        1.25,
        1.18
    ),
    (
        'DE',
        'Delaware',
        'state',
        0,
        21,
        989948,
        70176,
        1.08,
        1.05
    ),
    (
        'FL',
        'Florida',
        'state',
        0,
        21,
        21538187,
        57703,
        1.05,
        1.42
    ),
    (
        'GA',
        'Georgia',
        'state',
        0,
        21,
        10711908,
        61224,
        0.95,
        1.28
    ),
    (
        'HI',
        'Hawaii',
        'state',
        0,
        21,
        1455271,
        83102,
        1.65,
        0.98
    ),
    (
        'ID',
        'Idaho',
        'state',
        0,
        18,
        1839106,
        58915,
        0.92,
        0.82
    ),
    (
        'IL',
        'Illinois',
        'state',
        0,
        21,
        12812508,
        68428,
        1.04,
        1.25
    ),
    (
        'IN',
        'Indiana',
        'state',
        0,
        21,
        6785528,
        57603,
        0.91,
        0.92
    ),
    (
        'IA',
        'Iowa',
        'state',
        0,
        21,
        3190369,
        61691,
        0.90,
        0.78
    ),
    (
        'KS',
        'Kansas',
        'state',
        0,
        21,
        2937880,
        59597,
        0.89,
        0.82
    ),
    (
        'KY',
        'Kentucky',
        'state',
        0,
        18,
        4505836,
        52238,
        0.87,
        0.88
    ),
    (
        'LA',
        'Louisiana',
        'state',
        1,
        18,
        4657757,
        51073,
        0.93,
        1.02
    ),
    (
        'ME',
        'Maine',
        'state',
        0,
        18,
        1362359,
        63332,
        1.08,
        0.85
    ),
    (
        'MD',
        'Maryland',
        'state',
        0,
        21,
        6177224,
        87063,
        1.30,
        1.22
    ),
    (
        'MA',
        'Massachusetts',
        'state',
        0,
        21,
        7029917,
        85843,
        1.38,
        1.32
    ),
    (
        'MI',
        'Michigan',
        'state',
        0,
        18,
        10077331,
        59234,
        0.93,
        1.05
    ),
    (
        'MN',
        'Minnesota',
        'state',
        0,
        21,
        5706494,
        73382,
        0.98,
        0.95
    ),
    (
        'MS',
        'Mississippi',
        'state',
        0,
        21,
        2961279,
        45792,
        0.83,
        0.90
    ),
    (
        'MO',
        'Missouri',
        'state',
        0,
        21,
        6154913,
        57290,
        0.90,
        0.95
    ),
    (
        'MT',
        'Montana',
        'state',
        0,
        21,
        1084225,
        57153,
        0.95,
        0.72
    ),
    (
        'NE',
        'Nebraska',
        'state',
        0,
        21,
        1961504,
        63229,
        0.91,
        0.80
    ),
    (
        'NV',
        'Nevada',
        'state',
        1,
        18,
        3104614,
        63276,
        1.05,
        1.48
    ),
    (
        'NH',
        'New Hampshire',
        'state',
        0,
        21,
        1377529,
        76768,
        1.18,
        0.92
    ),
    (
        'NJ',
        'New Jersey',
        'state',
        0,
        21,
        9288994,
        85751,
        1.32,
        1.38
    ),
    (
        'NM',
        'New Mexico',
        'state',
        1,
        21,
        2117522,
        51945,
        0.92,
        1.08
    ),
    (
        'NY',
        'New York',
        'state',
        0,
        21,
        20201249,
        72108,
        1.28,
        1.52
    ),
    (
        'NC',
        'North Carolina',
        'state',
        0,
        18,
        10439388,
        57341,
        0.96,
        1.12
    ),
    (
        'ND',
        'North Dakota',
        'state',
        0,
        21,
        779094,
        64894,
        0.92,
        0.68
    ),
    (
        'OH',
        'Ohio',
        'state',
        0,
        21,
        11799448,
        58116,
        0.91,
        1.05
    ),
    (
        'OK',
        'Oklahoma',
        'state',
        0,
        18,
        3959353,
        54449,
        0.88,
        0.92
    ),
    (
        'OR',
        'Oregon',
        'state',
        0,
        21,
        4237256,
        67058,
        1.12,
        1.18
    ),
    (
        'PA',
        'Pennsylvania',
        'state',
        0,
        21,
        13002700,
        63463,
        0.98,
        1.15
    ),
    (
        'RI',
        'Rhode Island',
        'state',
        0,
        18,
        1097379,
        70305,
        1.20,
        1.22
    ),
    (
        'SC',
        'South Carolina',
        'state',
        0,
        18,
        5118425,
        54864,
        0.93,
        1.05
    ),
    (
        'SD',
        'South Dakota',
        'state',
        0,
        18,
        886667,
        59533,
        0.88,
        0.72
    ),
    (
        'TN',
        'Tennessee',
        'state',
        0,
        21,
        6910840,
        54833,
        0.91,
        1.08
    ),
    (
        'TX',
        'Texas',
        'state',
        1,
        21,
        29145505,
        63826,
        0.96,
        1.22
    ),
    (
        'UT',
        'Utah',
        'state',
        0,
        21,
        3271616,
        70957,
        0.98,
        0.95
    ),
    (
        'VT',
        'Vermont',
        'state',
        0,
        18,
        643077,
        63001,
        1.12,
        0.82
    ),
    (
        'VA',
        'Virginia',
        'state',
        0,
        18,
        8631393,
        74222,
        1.08,
        1.12
    ),
    (
        'WA',
        'Washington',
        'state',
        1,
        21,
        7705281,
        78687,
        1.15,
        1.25
    ),
    (
        'WV',
        'West Virginia',
        'state',
        0,
        21,
        1793716,
        48037,
        0.82,
        0.78
    ),
    (
        'WI',
        'Wisconsin',
        'state',
        1,
        21,
        5893718,
        61747,
        0.93,
        0.92
    ),
    (
        'WY',
        'Wyoming',
        'state',
        0,
        21,
        576851,
        65003,
        0.92,
        0.68
    ),
    -- District
    (
        'DC',
        'District of Columbia',
        'district',
        0,
        21,
        689545,
        90842,
        1.52,
        1.35
    ),
    -- Territories
    (
        'PR',
        'Puerto Rico',
        'territory',
        0,
        21,
        3193694,
        21058,
        0.92,
        1.08
    ),
    (
        'VI',
        'US Virgin Islands',
        'territory',
        0,
        18,
        106977,
        37254,
        1.18,
        0.95
    ),
    (
        'GU',
        'Guam',
        'territory',
        0,
        18,
        168801,
        35600,
        1.12,
        0.88
    ),
    (
        'AS',
        'American Samoa',
        'territory',
        0,
        18,
        49710,
        28352,
        0.95,
        0.72
    ),
    (
        'MP',
        'Northern Mariana Islands',
        'territory',
        0,
        18,
        51659,
        31950,
        1.08,
        0.68
    );
-- ─────────────────────────────────────────────────────────────────────────────
-- customer_identity: SSN and identity metadata
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS customer_identity (
    customer_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    ssn_full TEXT NOT NULL,
    -- format: XXX-XX-XXXX (synthetic, not real)
    ssn_area TEXT NOT NULL,
    -- first 3 digits
    ssn_group TEXT NOT NULL,
    -- middle 2 digits
    ssn_serial TEXT NOT NULL,
    -- last 4 digits
    ssn_status TEXT NOT NULL DEFAULT 'valid',
    -- valid|synthetic|deceased|invalid
    identity_type TEXT NOT NULL DEFAULT 'natural_person',
    -- natural_person|synthetic|stolen
    date_of_birth TEXT NOT NULL,
    -- YYYY-MM-DD
    age_at_open INTEGER NOT NULL,
    -- age in years at account open tick
    ssn_shared_count INTEGER NOT NULL DEFAULT 0,
    -- how many customers share this SSN
    ssn_first_seen_tick INTEGER NOT NULL,
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);
CREATE INDEX IF NOT EXISTS idx_identity_ssn ON customer_identity(ssn_full, run_id);
CREATE INDEX IF NOT EXISTS idx_identity_run ON customer_identity(run_id);
-- ─────────────────────────────────────────────────────────────────────────────
-- customer_address: physical address with classification
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS customer_address (
    address_id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    street_address TEXT NOT NULL,
    city TEXT NOT NULL,
    state TEXT NOT NULL,
    zip_code TEXT NOT NULL,
    -- Classification
    address_type TEXT NOT NULL,
    -- residential|po_box|cmra|homeless_shelter|domestic_violence_shelter|commercial
    address_stability TEXT NOT NULL,
    -- stable|transient|temporary
    verification_status TEXT NOT NULL DEFAULT 'unverified',
    -- unverified|verified|failed
    delivery_point TEXT,
    -- residential|commercial|po_box|undeliverable
    dwelling_type TEXT,
    -- single_family|apartment|condo|shelter|hotel|mobile
    occupant_count INTEGER NOT NULL DEFAULT 1,
    -- how many customers at this exact address
    first_seen_tick INTEGER NOT NULL,
    -- Risk flags
    is_high_risk INTEGER NOT NULL DEFAULT 0,
    is_protected_class INTEGER NOT NULL DEFAULT 0,
    -- fair housing / ECOA protected attributes present
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);
CREATE INDEX IF NOT EXISTS idx_addr_sharing ON customer_address(state, zip_code, city, street_address, run_id);
CREATE INDEX IF NOT EXISTS idx_addr_customer ON customer_address(run_id, customer_id);
-- ─────────────────────────────────────────────────────────────────────────────
-- customer_phone: phone numbers with fraud risk attributes
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS customer_phone (
    phone_id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    country_code TEXT NOT NULL DEFAULT '+1',
    area_code TEXT NOT NULL,
    exchange_code TEXT NOT NULL,
    subscriber_number TEXT NOT NULL,
    full_number TEXT NOT NULL,
    phone_type TEXT NOT NULL,
    -- mobile|home|work|voip|fax
    is_primary INTEGER NOT NULL DEFAULT 0,
    is_verified INTEGER NOT NULL DEFAULT 0,
    voip_indicator INTEGER NOT NULL DEFAULT 0,
    burner_phone_score REAL NOT NULL DEFAULT 0.0,
    -- 0.0–1.0
    carrier TEXT,
    is_ported INTEGER NOT NULL DEFAULT 0,
    -- recently transferred between carriers
    first_seen_tick INTEGER NOT NULL,
    sms_failures INTEGER NOT NULL DEFAULT 0,
    -- Sharing: updated incrementally when multiple customers share the same number
    customer_count INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);
CREATE INDEX IF NOT EXISTS idx_phone_sharing ON customer_phone(full_number, run_id);
CREATE INDEX IF NOT EXISTS idx_phone_customer ON customer_phone(run_id, customer_id);
-- ─────────────────────────────────────────────────────────────────────────────
-- Extend customer table with Tier 1 attributes
-- SQLite only allows one ADD COLUMN per statement
-- ─────────────────────────────────────────────────────────────────────────────
ALTER TABLE customer
ADD COLUMN is_vulnerable INTEGER NOT NULL DEFAULT 0;
ALTER TABLE customer
ADD COLUMN vulnerability_type TEXT;
ALTER TABLE customer
ADD COLUMN state_code TEXT;
-- references state_config(state_code)