use axum::extract::{Path, Query, State};
use axum::Json;

use crate::error::AppError;
use crate::ledger;
use crate::model::{
    AccountBalance, EntryView, JournalQuery, LineView, NewEntry, PostedEntry, ReverseRequest,
};
use crate::AppState;

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<NewEntry>,
) -> Result<Json<PostedEntry>, AppError> {
    Ok(Json(ledger::post_entry(&state.pool, req).await?))
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<EntryView>, AppError> {
    Ok(Json(ledger::get_entry(&state.pool, id).await?))
}

pub async fn reverse(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    body: Option<Json<ReverseRequest>>,
) -> Result<Json<PostedEntry>, AppError> {
    let req = body.map(|Json(b)| b).unwrap_or_default();
    Ok(Json(ledger::reverse(&state.pool, id, req.reason, req.date).await?))
}

pub async fn journal(
    State(state): State<AppState>,
    Query(q): Query<JournalQuery>,
) -> Result<Json<Vec<LineView>>, AppError> {
    Ok(Json(ledger::journal(&state.pool, q.account).await?))
}

pub async fn balances(
    State(state): State<AppState>,
) -> Result<Json<Vec<AccountBalance>>, AppError> {
    Ok(Json(ledger::balances(&state.pool).await?))
}
