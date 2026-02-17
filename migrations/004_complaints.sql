-- FinCrime: The Desk — Migration 004: Complaints & Interactions

CREATE TABLE IF NOT EXISTS complaint (
    complaint_id    TEXT    PRIMARY KEY,
    run_id          TEXT    NOT NULL,
    customer_id     TEXT    NOT NULL,
    account_id      TEXT,
    tick_opened     INTEGER NOT NULL,
    tick_closed     INTEGER,
    product         TEXT    NOT NULL,
    issue           TEXT    NOT NULL,
    priority        TEXT    NOT NULL DEFAULT 'standard',
    status          TEXT    NOT NULL DEFAULT 'open',
    sla_due_tick    INTEGER NOT NULL,
    sla_breached    INTEGER NOT NULL DEFAULT 0,
    resolution_code TEXT,
    amount_refunded REAL    DEFAULT 0.0,
    udaap_flag      INTEGER DEFAULT 0,
    FOREIGN KEY (run_id)      REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);
CREATE INDEX IF NOT EXISTS idx_complaint_customer
    ON complaint (run_id, customer_id, status);
CREATE INDEX IF NOT EXISTS idx_complaint_status
    ON complaint (run_id, status, tick_opened);
CREATE INDEX IF NOT EXISTS idx_complaint_sla
    ON complaint (run_id, sla_due_tick, sla_breached);

CREATE TABLE IF NOT EXISTS interaction (
    interaction_id     TEXT    PRIMARY KEY,
    run_id             TEXT    NOT NULL,
    customer_id        TEXT    NOT NULL,
    tick               INTEGER NOT NULL,
    channel            TEXT    NOT NULL,
    interaction_type   TEXT    NOT NULL,
    complaint_id       TEXT,
    outcome            TEXT,
    satisfaction_delta REAL    DEFAULT 0.0,
    FOREIGN KEY (run_id)       REFERENCES run(run_id),
    FOREIGN KEY (customer_id)  REFERENCES customer(customer_id),
    FOREIGN KEY (complaint_id) REFERENCES complaint(complaint_id)
);
CREATE INDEX IF NOT EXISTS idx_interaction_customer
    ON interaction (run_id, customer_id, tick DESC);

CREATE TABLE IF NOT EXISTS complaint_aggregate (
    run_id               TEXT    NOT NULL,
    tick                 INTEGER NOT NULL,
    complaints_opened    INTEGER NOT NULL DEFAULT 0,
    complaints_closed    INTEGER NOT NULL DEFAULT 0,
    sla_breaches         INTEGER NOT NULL DEFAULT 0,
    avg_age_days         REAL    NOT NULL DEFAULT 0.0,
    backlog_count        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (run_id, tick)
);
