use axum::extract::State;
use axum::Json;

use crate::error::AppError;
use crate::model::Period;
use crate::AppState;

pub async fn get(State(state): State<AppState>) -> Result<Json<Option<Period>>, AppError> {
    let row = sqlx::query_as::<_, Period>("SELECT from_ym, to_ym FROM posting_period WHERE id = 1")
        .fetch_optional(&state.pool)
        .await?;
    Ok(Json(row))
}

pub async fn put(
    State(state): State<AppState>,
    Json(period): Json<Period>,
) -> Result<Json<Period>, AppError> {
    sqlx::query(
        "INSERT INTO posting_period (id, from_ym, to_ym) VALUES (1, $1, $2) \
         ON CONFLICT (id) DO UPDATE SET from_ym = EXCLUDED.from_ym, to_ym = EXCLUDED.to_ym",
    )
    .bind(period.from_ym)
    .bind(period.to_ym)
    .execute(&state.pool)
    .await?;
    Ok(Json(period))
}
