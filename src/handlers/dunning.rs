use axum::extract::{Path, State};
use axum::Json;

use crate::dunning;
use crate::error::AppError;
use crate::model::{DunningRunRequest, DunningRunResult};
use crate::AppState;

pub async fn run(
    State(state): State<AppState>,
    Json(req): Json<DunningRunRequest>,
) -> Result<Json<DunningRunResult>, AppError> {
    Ok(Json(dunning::run(&state.pool, req).await?))
}

pub async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<DunningRunResult>, AppError> {
    Ok(Json(dunning::get_run(&state.pool, &run_id).await?))
}
