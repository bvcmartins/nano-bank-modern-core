CREATE TABLE IF NOT EXISTS gl_account (
    code TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'CAD',
    open_item_managed BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS journal_entry (
    id BIGSERIAL PRIMARY KEY,
    entry_date DATE NOT NULL,
    currency TEXT NOT NULL DEFAULT 'CAD',
    fx_rate NUMERIC(18,6) NOT NULL DEFAULT 1,
    reference TEXT,
    description TEXT,
    reversal_of BIGINT REFERENCES journal_entry(id),
    reversed_by BIGINT REFERENCES journal_entry(id),
    reversal_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS journal_line (
    id BIGSERIAL PRIMARY KEY,
    entry_id BIGINT NOT NULL REFERENCES journal_entry(id),
    line_no INT NOT NULL,
    account TEXT NOT NULL REFERENCES gl_account(code),
    direction TEXT NOT NULL CHECK (direction IN ('debit','credit')),
    amount NUMERIC(18,2) NOT NULL CHECK (amount > 0),
    amount_local NUMERIC(18,2) NOT NULL,
    tax_code TEXT,
    terms TEXT,
    due_date DATE,
    open BOOLEAN NOT NULL DEFAULT FALSE,
    cleared_by BIGINT,
    cleared_on DATE,
    dunning_level INT,
    last_dunned DATE
);
CREATE INDEX IF NOT EXISTS idx_journal_line_entry ON journal_line(entry_id);
CREATE INDEX IF NOT EXISTS idx_journal_line_open ON journal_line(account, open) WHERE cleared_by IS NULL;

CREATE TABLE IF NOT EXISTS tax_code (
    code TEXT PRIMARY KEY,
    name TEXT,
    rate NUMERIC(7,3) NOT NULL,
    account TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS exchange_rate (
    from_ccy TEXT NOT NULL,
    to_ccy TEXT NOT NULL,
    rate NUMERIC(18,6) NOT NULL,
    as_of DATE NOT NULL,
    PRIMARY KEY (from_ccy, to_ccy, as_of)
);

CREATE TABLE IF NOT EXISTS payment_terms (
    code TEXT PRIMARY KEY,
    name TEXT,
    net_days INT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS dunning_level (
    level INT PRIMARY KEY,
    min_days INT NOT NULL,
    charge NUMERIC(18,2) NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS posting_period (
    id INT PRIMARY KEY,
    from_ym INT NOT NULL,
    to_ym INT NOT NULL
);

CREATE TABLE IF NOT EXISTS clearing (
    id BIGSERIAL PRIMARY KEY,
    clear_date DATE NOT NULL,
    account TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS dunning_notice (
    run_id TEXT NOT NULL,
    run_date DATE NOT NULL,
    account TEXT NOT NULL,
    level INT NOT NULL,
    total NUMERIC(18,2) NOT NULL,
    charge NUMERIC(18,2) NOT NULL DEFAULT 0,
    PRIMARY KEY (run_id, account)
);

CREATE TABLE IF NOT EXISTS dunning_notice_line (
    run_id TEXT NOT NULL,
    account TEXT NOT NULL,
    line_id BIGINT NOT NULL,
    level INT NOT NULL,
    net_due DATE,
    days_overdue INT,
    amount NUMERIC(18,2) NOT NULL,
    PRIMARY KEY (run_id, line_id)
);
