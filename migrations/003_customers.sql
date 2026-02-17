-- FinCrime: The Desk — Migration 003: Customers, Accounts, Transactions
CREATE TABLE IF NOT EXISTS customer (
    customer_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    segment TEXT NOT NULL,
    income_band TEXT NOT NULL,
    risk_band TEXT NOT NULL DEFAULT 'low',
    open_tick INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    churn_risk REAL NOT NULL DEFAULT 0.0,
    satisfaction REAL NOT NULL DEFAULT 0.8,
    -- Behavioral model snapshot (from segment config)
    monthly_txn_mean REAL NOT NULL,
    cash_intensity REAL NOT NULL,
    payroll_amount REAL NOT NULL DEFAULT 0.0,
    has_payroll INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_customer_run ON customer (run_id, status);
CREATE TABLE IF NOT EXISTS account (
    account_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    balance REAL NOT NULL DEFAULT 0.0,
    open_tick INTEGER NOT NULL,
    close_tick INTEGER,
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (customer_id) REFERENCES customer(customer_id)
);
CREATE INDEX IF NOT EXISTS idx_account_customer ON account (run_id, customer_id, status);
CREATE TABLE IF NOT EXISTS transactions (
    txn_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    amount REAL NOT NULL,
    direction TEXT NOT NULL,
    category TEXT NOT NULL,
    counterparty TEXT,
    fraud_flag INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (run_id) REFERENCES run(run_id),
    FOREIGN KEY (account_id) REFERENCES account(account_id)
);
CREATE INDEX IF NOT EXISTS idx_txn_account ON transactions (run_id, account_id, tick);
CREATE INDEX IF NOT EXISTS idx_txn_tick ON transactions (run_id, tick);
CREATE TABLE IF NOT EXISTS daily_aggregate (
    run_id TEXT NOT NULL,
    tick INTEGER NOT NULL,
    txn_count INTEGER NOT NULL DEFAULT 0,
    txn_volume REAL NOT NULL DEFAULT 0.0,
    fee_income REAL NOT NULL DEFAULT 0.0,
    overdraft_events INTEGER NOT NULL DEFAULT 0,
    nsf_events INTEGER NOT NULL DEFAULT 0,
    new_customers INTEGER NOT NULL DEFAULT 0,
    churned_customers INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (run_id, tick)
);
