# nano-bank modern core

A clean, modern general-ledger service for `nano-bank` — the Rust **peer** of
`nano-bank-legacy-core`. Same accounting capabilities, none of the legacy
ceremony: one `journal_entry` + `journal_line` model (no header/item/journal
triplication, no number ranges, no cryptic table names), business rules enforced
in Rust, and a plain REST API.

`nano-bank` talks to this service or to the legacy core interchangeably, behind a
single `Ledger` port (`CORE_BACKEND=modern|legacy`).

## Capabilities (parity with the legacy core)

- **General ledger**: a chart of GL accounts (`gl_account`) and balanced
  double-entry posting.
- **Posting** `POST /entries`: validates the entry balances (debits = credits),
  generates a tax line from a line's `tax_code`, translates a foreign currency to
  CAD via `exchange_rate`, and guards the open posting period.
- **Reversal** `POST /entries/{id}/reverse`: posts a mirror entry, cross-links both.
- **Open items & clearing**: lines on open-item-managed accounts (AR/AP) stay open
  until a clearing that nets to zero (`GET /open-items`, `POST /clearings`).
- **Dunning** `POST /dunning-runs` (`test` = proposal): ages open receivable items
  against payment terms, assigns dunning levels, and produces one notice per account.

## Run it

```bash
# database only (fast inner loop — run the app with cargo)
docker compose up -d db
DATABASE_URL=postgres://core:core@localhost:5435/modern_core cargo run

# or the whole thing in containers
docker compose up -d --build
```

Schema and seed (`resources/schema.sql`, `resources/seed.sql`) are applied
idempotently at startup. Service listens on `:8091`.

## Quick check

```bash
curl localhost:8091/health
curl localhost:8091/accounts

# post: debit Bank / credit Revenue 100
curl -X POST localhost:8091/entries -H 'content-type: application/json' -d '{
  "lines":[
    {"account":"BANK","direction":"debit","amount":100.00},
    {"account":"REVENUE","direction":"credit","amount":100.00}
  ]}'

curl localhost:8091/balances
```

## Seeded accounts

`BANK`, `AR` (open-item), `AP` (open-item), `REVENUE`, `EXPENSE`, `INPUT_TAX`,
`OUTPUT_TAX`. These are the semantic accounts the `nano-bank` `Ledger` port maps
onto — the legacy adapter maps the same names onto the legacy core's
`0000xxxxxx` account numbers.
