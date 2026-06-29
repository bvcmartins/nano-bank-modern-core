//! Open-item clearing: a set of open items on the same account that nets to zero
//! is assigned a clearing document, which marks each item as cleared.

use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::error::AppError;
use crate::model::{ClearRequest, ClearingResult, OpenItem};

pub async fn open_items(
    pool: &PgPool,
    account: Option<String>,
) -> Result<Vec<OpenItem>, AppError> {
    let rows = if let Some(account) = account {
        sqlx::query_as::<_, OpenItem>(
            "SELECT id, entry_id, account, direction, amount_local, due_date \
             FROM journal_line WHERE open AND cleared_by IS NULL AND account = $1 \
             ORDER BY id",
        )
        .bind(account)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, OpenItem>(
            "SELECT id, entry_id, account, direction, amount_local, due_date \
             FROM journal_line WHERE open AND cleared_by IS NULL \
             ORDER BY account, id",
        )
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

#[derive(sqlx::FromRow)]
struct OpenLine {
    account: String,
    direction: String,
    amount_local: Decimal,
    open: bool,
    cleared_by: Option<i64>,
}

pub async fn clear(pool: &PgPool, req: ClearRequest) -> Result<ClearingResult, AppError> {
    if req.items.is_empty() {
        return Err(AppError::BadRequest("no items selected for clearing".into()));
    }
    let date = req.date.unwrap_or_else(|| Utc::now().date_naive());

    let mut account: Option<String> = None;
    let mut residual = Decimal::ZERO;
    for &line_id in &req.items {
        let line: OpenLine = sqlx::query_as(
            "SELECT account, direction, amount_local, open, cleared_by \
             FROM journal_line WHERE id = $1",
        )
        .bind(line_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("item {line_id} not found")))?;

        if !line.open {
            return Err(AppError::BadRequest(format!(
                "item {line_id} is not open-item managed"
            )));
        }
        if line.cleared_by.is_some() {
            return Err(AppError::BadRequest(format!(
                "item {line_id} is already cleared"
            )));
        }
        match &account {
            None => account = Some(line.account.clone()),
            Some(a) if a != &line.account => {
                return Err(AppError::BadRequest(
                    "all items to be cleared must be on the same account".into(),
                ))
            }
            _ => {}
        }
        residual += if line.direction == "debit" {
            line.amount_local
        } else {
            -line.amount_local
        };
    }

    if residual != Decimal::ZERO {
        return Err(AppError::Unprocessable(format!(
            "selected items do not net to zero (residual {residual} CAD)"
        )));
    }
    let account = account.expect("non-empty items guarantee an account");

    let mut tx = pool.begin().await?;
    let clearing_id: i64 =
        sqlx::query_scalar("INSERT INTO clearing (clear_date, account) VALUES ($1, $2) RETURNING id")
            .bind(date)
            .bind(&account)
            .fetch_one(&mut *tx)
            .await?;
    for &line_id in &req.items {
        sqlx::query("UPDATE journal_line SET cleared_by = $1, cleared_on = $2 WHERE id = $3")
            .bind(clearing_id)
            .bind(date)
            .bind(line_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(ClearingResult {
        id: clearing_id,
        account,
        cleared: req.items.len(),
    })
}
