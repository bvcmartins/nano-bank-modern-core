//! Dunning: age the open receivable items, assign a dunning level to each overdue
//! item, group them into one notice per account, and (unless it is a proposal)
//! record the notices and raise the dunning level on the items.

use std::collections::HashMap;

use chrono::{Duration, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::error::AppError;
use crate::model::{DunningItem, DunningNotice, DunningRunRequest, DunningRunResult};

/// Days that must elapse before an item is dunned again at the same level.
const INTERVAL_DAYS: i64 = 14;

#[derive(sqlx::FromRow)]
struct Candidate {
    id: i64,
    account: String,
    amount_local: Decimal,
    due_date: Option<NaiveDate>,
    terms: Option<String>,
    dunning_level: Option<i32>,
    last_dunned: Option<NaiveDate>,
    entry_date: NaiveDate,
}

struct Dunnable {
    id: i64,
    account: String,
    amount: Decimal,
    level: i32,
    net_due: NaiveDate,
    days_overdue: i64,
}

pub async fn run(pool: &PgPool, req: DunningRunRequest) -> Result<DunningRunResult, AppError> {
    if req.run_id.trim().is_empty() {
        return Err(AppError::BadRequest("run_id must be supplied".into()));
    }
    let run_date = req.run_date.unwrap_or_else(|| Utc::now().date_naive());

    // (level, min_days, charge) ascending.
    let levels: Vec<(i32, i32, Decimal)> =
        sqlx::query_as("SELECT level, min_days, charge FROM dunning_level ORDER BY level")
            .fetch_all(pool)
            .await?;
    if levels.is_empty() {
        return Err(AppError::BadRequest("no dunning levels configured".into()));
    }

    let net_days: HashMap<String, i32> =
        sqlx::query_as::<_, (String, i32)>("SELECT code, net_days FROM payment_terms")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();

    let candidates: Vec<Candidate> = sqlx::query_as(
        "SELECT jl.id, jl.account, jl.amount_local, jl.due_date, jl.terms, \
                jl.dunning_level, jl.last_dunned, je.entry_date \
         FROM journal_line jl \
         JOIN journal_entry je ON je.id = jl.entry_id \
         JOIN gl_account g ON g.code = jl.account \
         WHERE g.open_item_managed AND jl.direction = 'debit' \
               AND jl.open AND jl.cleared_by IS NULL \
         ORDER BY jl.account, jl.id",
    )
    .fetch_all(pool)
    .await?;

    // Age each item and keep the ones that should be dunned now.
    let mut dunnable: Vec<Dunnable> = Vec::new();
    for c in &candidates {
        let baseline = c.due_date.unwrap_or(c.entry_date);
        let net = c.terms.as_deref().and_then(|t| net_days.get(t)).copied().unwrap_or(0);
        let net_due = baseline + Duration::days(net as i64);
        let days_overdue = (run_date - net_due).num_days();

        let target = level_for(&levels, days_overdue);
        if target == 0 {
            continue;
        }
        let current = c.dunning_level.unwrap_or(0);
        let escalate = target > current;
        let repeat = target == current
            && current >= 1
            && c.last_dunned
                .map_or(true, |d| (run_date - d).num_days() >= INTERVAL_DAYS);
        if !escalate && !repeat {
            continue;
        }
        dunnable.push(Dunnable {
            id: c.id,
            account: c.account.clone(),
            amount: c.amount_local,
            level: target,
            net_due,
            days_overdue,
        });
    }

    // Group into one notice per account.
    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<&Dunnable>> = HashMap::new();
    for d in &dunnable {
        if !grouped.contains_key(&d.account) {
            order.push(d.account.clone());
            grouped.insert(d.account.clone(), Vec::new());
        }
        grouped.get_mut(&d.account).unwrap().push(d);
    }

    let mut notices: Vec<DunningNotice> = Vec::new();
    for account in &order {
        let items = &grouped[account];
        let level = items.iter().map(|d| d.level).max().unwrap_or(0);
        let total: Decimal = items.iter().map(|d| d.amount).sum();
        let charge = levels
            .iter()
            .find(|(l, _, _)| *l == level)
            .map(|(_, _, c)| *c)
            .unwrap_or(Decimal::ZERO);

        notices.push(DunningNotice {
            account: account.clone(),
            level,
            total,
            charge,
            items: items
                .iter()
                .map(|d| DunningItem {
                    line_id: d.id,
                    level: d.level,
                    net_due: d.net_due,
                    days_overdue: d.days_overdue,
                    amount: d.amount,
                })
                .collect(),
        });
    }

    if !req.test {
        persist(pool, &req.run_id, run_date, &notices).await?;
    }

    Ok(DunningRunResult {
        run_id: req.run_id,
        run_date,
        test: req.test,
        notices,
    })
}

async fn persist(
    pool: &PgPool,
    run_id: &str,
    run_date: NaiveDate,
    notices: &[DunningNotice],
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    for notice in notices {
        sqlx::query(
            "INSERT INTO dunning_notice (run_id, run_date, account, level, total, charge) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(run_id)
        .bind(run_date)
        .bind(&notice.account)
        .bind(notice.level)
        .bind(notice.total)
        .bind(notice.charge)
        .execute(&mut *tx)
        .await?;
        for item in &notice.items {
            sqlx::query(
                "INSERT INTO dunning_notice_line \
                 (run_id, account, line_id, level, net_due, days_overdue, amount) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(run_id)
            .bind(&notice.account)
            .bind(item.line_id)
            .bind(item.level)
            .bind(item.net_due)
            .bind(item.days_overdue as i32)
            .bind(item.amount)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE journal_line SET dunning_level = $1, last_dunned = $2 WHERE id = $3",
            )
            .bind(item.level)
            .bind(run_date)
            .bind(item.line_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

pub async fn get_run(pool: &PgPool, run_id: &str) -> Result<DunningRunResult, AppError> {
    let headers: Vec<(NaiveDate, String, i32, Decimal, Decimal)> = sqlx::query_as(
        "SELECT run_date, account, level, total, charge FROM dunning_notice \
         WHERE run_id = $1 ORDER BY account",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;

    if headers.is_empty() {
        return Err(AppError::NotFound(format!("dunning run {run_id} not found")));
    }
    let run_date = headers[0].0;

    let mut notices = Vec::new();
    for (_, account, level, total, charge) in headers {
        let items: Vec<(i64, i32, NaiveDate, i32, Decimal)> = sqlx::query_as(
            "SELECT line_id, level, net_due, days_overdue, amount FROM dunning_notice_line \
             WHERE run_id = $1 AND account = $2 ORDER BY line_id",
        )
        .bind(run_id)
        .bind(&account)
        .fetch_all(pool)
        .await?;
        notices.push(DunningNotice {
            account,
            level,
            total,
            charge,
            items: items
                .into_iter()
                .map(|(line_id, level, net_due, days_overdue, amount)| DunningItem {
                    line_id,
                    level,
                    net_due,
                    days_overdue: days_overdue as i64,
                    amount,
                })
                .collect(),
        });
    }

    Ok(DunningRunResult {
        run_id: run_id.to_string(),
        run_date,
        test: false,
        notices,
    })
}

/// The highest configured level whose minimum days in arrears is reached; 0 if none.
fn level_for(levels: &[(i32, i32, Decimal)], days_overdue: i64) -> i32 {
    let mut level = 0;
    for (l, min_days, _) in levels {
        if days_overdue >= *min_days as i64 {
            level = *l;
        }
    }
    level
}
