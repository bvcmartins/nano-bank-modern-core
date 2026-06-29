use axum::extract::{Query, State};
use axum::Json;

use crate::clearing;
use crate::error::AppError;
use crate::model::{ClearRequest, ClearingResult, JournalQuery, OpenItem};
use crate::AppState;

pub async fn open_items(
    State(state): State<AppState>,
    Query(q): Query<JournalQuery>,
) -> Result<Json<Vec<OpenItem>>, AppError> {
    Ok(Json(clearing::open_items(&state.pool, q.account).await?))
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<ClearRequest>,
) -> Result<Json<ClearingResult>, AppError> {
    Ok(Json(clearing::clear(&state.pool, req).await?))
}
