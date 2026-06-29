//! The posting engine: validate, generate tax lines, translate currency, check
//! that the entry balances, and persist the journal. Reversal posts a mirror
//! entry and cross-links the two.

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::error::AppError;
use crate::model::{
    AccountBalance, EntryHeader, EntryView, GlAccount, LineView, NewEntry, NewLine, PostedEntry,
};

const BASE_CCY: &str = "CAD";

/// An effective line after tax generation, ready to persist.
struct EffLine {
    account: String,
    direction: String,
    amount: Decimal,
    tax_code: Option<String>,
    terms: Option<String>,
    due_date: Option<NaiveDate>,
    open: bool,
}

#[derive(sqlx::FromRow)]
struct TaxCodeRow {
    rate: Decimal,
    account: String,
}

/// Post a balanced journal entry.
pub async fn post_entry(pool: &PgPool, req: NewEntry) -> Result<PostedEntry, AppError> {
    if req.lines.len() < 2 {
        return Err(AppError::BadRequest(
            "an entry needs at least two lines".into(),
        ));
    }
    let date = req.date.unwrap_or_else(|| Utc::now().date_naive());
    let currency = req
        .currency
        .clone()
        .unwrap_or_else(|| BASE_CCY.to_string());

    check_period_open(pool, date).await?;
    let fx_rate = rate_to_base(pool, &currency, date).await?;

    // Build effective lines, generating a tax line for any line that carries a code.
    let mut eff: Vec<EffLine> = Vec::new();
    for line in &req.lines {
        validate_direction(&line.direction, &line.account)?;
        if line.amount <= Decimal::ZERO {
            return Err(AppError::BadRequest(format!(
                "amount must be positive on {}",
                line.account
            )));
        }
        let account = load_account(pool, &line.account).await?;
        eff.push(EffLine {
            account: account.code.clone(),
            direction: line.direction.clone(),
            amount: line.amount,
            tax_code: line.tax_code.clone(),
            terms: line.terms.clone(),
            due_date: line.due_date,
            open: account.open_item_managed,
        });
        if let Some(code) = &line.tax_code {
            if let Some(tax) = tax_line(pool, code, line).await? {
                eff.push(tax);
            }
        }
    }

    // Balance check in the document currency.
    let (mut debit, mut credit) = (Decimal::ZERO, Decimal::ZERO);
    for line in &eff {
        if line.direction == "debit" {
            debit += line.amount;
        } else {
            credit += line.amount;
        }
    }
    if debit != credit {
        return Err(AppError::Unprocessable(format!(
            "entry does not balance: debit {debit} != credit {credit} {currency}"
        )));
    }

    // Persist header + lines atomically.
    let mut tx = pool.begin().await?;
    let entry_id: i64 = sqlx::query_scalar(
        "INSERT INTO journal_entry (entry_date, currency, fx_rate, reference, description) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(date)
    .bind(&currency)
    .bind(fx_rate)
    .bind(&req.reference)
    .bind(&req.description)
    .fetch_one(&mut *tx)
    .await?;

    let mut line_no = 0i32;
    for line in &eff {
        line_no += 1;
        let amount_local = (line.amount * fx_rate).round_dp(2);
        let due = if line.open {
            Some(line.due_date.unwrap_or(date))
        } else {
            None
        };
        let terms = if line.open { line.terms.clone() } else { None };
        sqlx::query(
            "INSERT INTO journal_line \
             (entry_id, line_no, account, direction, amount, amount_local, tax_code, terms, due_date, open) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(entry_id)
        .bind(line_no)
        .bind(&line.account)
        .bind(&line.direction)
        .bind(line.amount)
        .bind(amount_local)
        .bind(&line.tax_code)
        .bind(&terms)
        .bind(due)
        .bind(line.open)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(PostedEntry {
        id: entry_id,
        currency,
        fx_rate,
        lines: eff.len(),
    })
}

/// Reverse an entry by posting a mirror (debit/credit flipped) and cross-linking.
pub async fn reverse(
    pool: &PgPool,
    id: i64,
    reason: Option<String>,
    date: Option<NaiveDate>,
) -> Result<PostedEntry, AppError> {
    let header: EntryHeader = sqlx::query_as(
        "SELECT id, entry_date, currency, fx_rate, reference, description, \
                reversal_of, reversed_by, reversal_reason \
         FROM journal_entry WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("entry {id} not found")))?;

    if header.reversed_by.is_some() {
        return Err(AppError::Conflict(format!(
            "entry {id} is already reversed by entry {}",
            header.reversed_by.unwrap()
        )));
    }

    let originals: Vec<(String, String, Decimal)> = sqlx::query_as(
        "SELECT account, direction, amount FROM journal_line WHERE entry_id = $1 ORDER BY line_no",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let lines: Vec<NewLine> = originals
        .into_iter()
        .map(|(account, direction, amount)| NewLine {
            account,
            direction: flip(&direction),
            amount,
            tax_code: None,
            terms: None,
            due_date: None,
        })
        .collect();

    let posted = post_entry(
        pool,
        NewEntry {
            date: Some(date.unwrap_or(header.entry_date)),
            currency: Some(header.currency.clone()),
            reference: Some(format!("reversal of {id}")),
            description: Some(format!("Reversal of entry {id}")),
            lines,
        },
    )
    .await?;

    // Cross-link the two entries.
    sqlx::query("UPDATE journal_entry SET reversal_of = $1, reversal_reason = $2 WHERE id = $3")
        .bind(id)
        .bind(&reason)
        .bind(posted.id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE journal_entry SET reversed_by = $1 WHERE id = $2")
        .bind(posted.id)
        .bind(id)
        .execute(pool)
        .await?;

    Ok(posted)
}

pub async fn get_entry(pool: &PgPool, id: i64) -> Result<EntryView, AppError> {
    let header: EntryHeader = sqlx::query_as(
        "SELECT id, entry_date, currency, fx_rate, reference, description, \
                reversal_of, reversed_by, reversal_reason \
         FROM journal_entry WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("entry {id} not found")))?;

    let lines = line_views(pool, Some(("entry_id", id)), None).await?;
    Ok(EntryView { header, lines })
}

pub async fn journal(pool: &PgPool, account: Option<String>) -> Result<Vec<LineView>, AppError> {
    line_views(pool, None, account).await
}

pub async fn balances(pool: &PgPool) -> Result<Vec<AccountBalance>, AppError> {
    let rows = sqlx::query_as::<_, AccountBalance>(
        "SELECT account, \
                COALESCE(SUM(CASE WHEN direction = 'debit' THEN amount_local ELSE -amount_local END), 0) AS balance \
         FROM journal_line GROUP BY account ORDER BY account",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const LINE_COLUMNS: &str = "id, entry_id, line_no, account, direction, amount, amount_local, \
    tax_code, due_date, open, cleared_by, dunning_level";

/// Load line views filtered either by an entry id or an account (or neither).
async fn line_views(
    pool: &PgPool,
    by_entry: Option<(&str, i64)>,
    by_account: Option<String>,
) -> Result<Vec<LineView>, AppError> {
    if let Some((_, entry_id)) = by_entry {
        return Ok(sqlx::query_as::<_, LineView>(&format!(
            "SELECT {LINE_COLUMNS} FROM journal_line WHERE entry_id = $1 ORDER BY line_no"
        ))
        .bind(entry_id)
        .fetch_all(pool)
        .await?);
    }
    if let Some(account) = by_account {
        return Ok(sqlx::query_as::<_, LineView>(&format!(
            "SELECT {LINE_COLUMNS} FROM journal_line WHERE account = $1 ORDER BY id"
        ))
        .bind(account)
        .fetch_all(pool)
        .await?);
    }
    Ok(sqlx::query_as::<_, LineView>(&format!(
        "SELECT {LINE_COLUMNS} FROM journal_line ORDER BY id"
    ))
    .fetch_all(pool)
    .await?)
}

fn validate_direction(direction: &str, account: &str) -> Result<(), AppError> {
    if direction != "debit" && direction != "credit" {
        return Err(AppError::BadRequest(format!(
            "direction must be 'debit' or 'credit' on {account}"
        )));
    }
    Ok(())
}

fn flip(direction: &str) -> String {
    if direction == "debit" { "credit" } else { "debit" }.to_string()
}

async fn load_account(pool: &PgPool, code: &str) -> Result<GlAccount, AppError> {
    sqlx::query_as::<_, GlAccount>(
        "SELECT code, name, kind, currency, open_item_managed FROM gl_account WHERE code = $1",
    )
    .bind(code)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest(format!("GL account {code} does not exist")))
}

/// Generate the tax line for a base line carrying a tax code: tax = base * rate,
/// posted to the tax account on the same side. `None` when the rate yields zero.
async fn tax_line(pool: &PgPool, code: &str, base: &NewLine) -> Result<Option<EffLine>, AppError> {
    let tax: TaxCodeRow =
        sqlx::query_as("SELECT rate, account FROM tax_code WHERE code = $1")
            .bind(code)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::BadRequest(format!("tax code {code} does not exist")))?;
    let amount = (base.amount * tax.rate / Decimal::ONE_HUNDRED).round_dp(2);
    if amount <= Decimal::ZERO {
        return Ok(None);
    }
    let account = load_account(pool, &tax.account).await?;
    Ok(Some(EffLine {
        account: account.code,
        direction: base.direction.clone(),
        amount,
        tax_code: Some(code.to_string()),
        terms: None,
        due_date: None,
        open: account.open_item_managed,
    }))
}

async fn rate_to_base(pool: &PgPool, currency: &str, date: NaiveDate) -> Result<Decimal, AppError> {
    if currency == BASE_CCY {
        return Ok(Decimal::ONE);
    }
    sqlx::query_scalar::<_, Decimal>(
        "SELECT rate FROM exchange_rate \
         WHERE from_ccy = $1 AND to_ccy = $2 AND as_of <= $3 \
         ORDER BY as_of DESC LIMIT 1",
    )
    .bind(currency)
    .bind(BASE_CCY)
    .bind(date)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Unprocessable(format!("no exchange rate for {currency}->{BASE_CCY}")))
}

async fn check_period_open(pool: &PgPool, date: NaiveDate) -> Result<(), AppError> {
    let window: Option<(i32, i32)> =
        sqlx::query_as("SELECT from_ym, to_ym FROM posting_period WHERE id = 1")
            .fetch_optional(pool)
            .await?;
    if let Some((from_ym, to_ym)) = window {
        let target = date.year() * 100 + date.month() as i32;
        if target < from_ym || target > to_ym {
            return Err(AppError::Unprocessable(format!(
                "posting period {target} is not open"
            )));
        }
    }
    Ok(())
}
