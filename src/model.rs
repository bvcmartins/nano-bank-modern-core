//! Request/response DTOs and database row types for the ledger API.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Posting
// ---------------------------------------------------------------------------

/// A balanced journal entry to post.
#[derive(Debug, Deserialize)]
pub struct NewEntry {
    pub date: Option<NaiveDate>,
    pub currency: Option<String>,
    pub reference: Option<String>,
    pub description: Option<String>,
    pub lines: Vec<NewLine>,
}

/// One line of an entry. `tax_code` generates a tax line; `terms`/`due_date`
/// drive the due date used by dunning (only meaningful on open-item accounts).
#[derive(Debug, Deserialize)]
pub struct NewLine {
    pub account: String,
    pub direction: String, // "debit" | "credit"
    pub amount: Decimal,
    #[serde(default)]
    pub tax_code: Option<String>,
    #[serde(default)]
    pub terms: Option<String>,
    #[serde(default)]
    pub due_date: Option<NaiveDate>,
}

/// Result of posting (or reversing) an entry.
#[derive(Debug, Serialize)]
pub struct PostedEntry {
    pub id: i64,
    pub currency: String,
    pub fx_rate: Decimal,
    pub lines: usize,
}

#[derive(Debug, Default, Deserialize)]
pub struct ReverseRequest {
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub date: Option<NaiveDate>,
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct JournalQuery {
    #[serde(default)]
    pub account: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GlAccount {
    pub code: String,
    pub name: String,
    pub kind: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub open_item_managed: bool,
}

fn default_currency() -> String {
    "CAD".to_string()
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AccountBalance {
    pub account: String,
    pub balance: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LineView {
    pub id: i64,
    pub entry_id: i64,
    pub line_no: i32,
    pub account: String,
    pub direction: String,
    pub amount: Decimal,
    pub amount_local: Decimal,
    pub tax_code: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub open: bool,
    pub cleared_by: Option<i64>,
    pub dunning_level: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EntryHeader {
    pub id: i64,
    pub entry_date: NaiveDate,
    pub currency: String,
    pub fx_rate: Decimal,
    pub reference: Option<String>,
    pub description: Option<String>,
    pub reversal_of: Option<i64>,
    pub reversed_by: Option<i64>,
    pub reversal_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EntryView {
    #[serde(flatten)]
    pub header: EntryHeader,
    pub lines: Vec<LineView>,
}

// ---------------------------------------------------------------------------
// Clearing
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ClearRequest {
    #[serde(default)]
    pub date: Option<NaiveDate>,
    pub items: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct ClearingResult {
    pub id: i64,
    pub account: String,
    pub cleared: usize,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OpenItem {
    pub id: i64,
    pub entry_id: i64,
    pub account: String,
    pub direction: String,
    pub amount_local: Decimal,
    pub due_date: Option<NaiveDate>,
}

// ---------------------------------------------------------------------------
// Dunning
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DunningRunRequest {
    pub run_id: String,
    #[serde(default)]
    pub run_date: Option<NaiveDate>,
    #[serde(default)]
    pub test: bool,
}

#[derive(Debug, Serialize)]
pub struct DunningRunResult {
    pub run_id: String,
    pub run_date: NaiveDate,
    pub test: bool,
    pub notices: Vec<DunningNotice>,
}

#[derive(Debug, Serialize)]
pub struct DunningNotice {
    pub account: String,
    pub level: i32,
    pub total: Decimal,
    pub charge: Decimal,
    pub items: Vec<DunningItem>,
}

#[derive(Debug, Serialize)]
pub struct DunningItem {
    pub line_id: i64,
    pub level: i32,
    pub net_due: NaiveDate,
    pub days_overdue: i64,
    pub amount: Decimal,
}

// ---------------------------------------------------------------------------
// Periods
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Period {
    pub from_ym: i32,
    pub to_ym: i32,
}
