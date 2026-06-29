INSERT INTO gl_account (code, name, kind, currency, open_item_managed) VALUES
    ('BANK',       'Bank account',         'asset',     'CAD', FALSE),
    ('AR',         'Accounts receivable',  'asset',     'CAD', TRUE),
    ('AP',         'Accounts payable',     'liability', 'CAD', TRUE),
    ('REVENUE',    'Revenue',              'revenue',   'CAD', FALSE),
    ('EXPENSE',    'Operating expenses',   'expense',   'CAD', FALSE),
    ('INPUT_TAX',  'Input tax',            'asset',     'CAD', FALSE),
    ('OUTPUT_TAX', 'Output tax',           'liability', 'CAD', FALSE)
ON CONFLICT (code) DO NOTHING;

INSERT INTO tax_code (code, name, rate, account) VALUES
    ('GST', 'GST 5%',  5.000,  'OUTPUT_TAX'),
    ('HST', 'HST 13%', 13.000, 'OUTPUT_TAX')
ON CONFLICT (code) DO NOTHING;

INSERT INTO exchange_rate (from_ccy, to_ccy, rate, as_of) VALUES
    ('USD', 'CAD', 1.370000, DATE '2026-01-01'),
    ('EUR', 'CAD', 1.460000, DATE '2026-01-01')
ON CONFLICT DO NOTHING;

INSERT INTO payment_terms (code, name, net_days) VALUES
    ('IMMEDIATE', 'Due immediately', 0),
    ('NET14',     'Net 14 days',     14),
    ('NET30',     'Net 30 days',     30)
ON CONFLICT (code) DO NOTHING;

INSERT INTO dunning_level (level, min_days, charge) VALUES
    (1, 1,  0.00),
    (2, 15, 10.00),
    (3, 30, 25.00)
ON CONFLICT (level) DO NOTHING;

INSERT INTO posting_period (id, from_ym, to_ym) VALUES
    (1, 202601, 202612)
ON CONFLICT (id) DO NOTHING;
