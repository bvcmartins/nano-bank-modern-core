use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::error::AppError;
use crate::model::GlAccount;
use crate::AppState;

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<GlAccount>>, AppError> {
    let rows = sqlx::query_as::<_, GlAccount>(
        "SELECT code, name, kind, currency, open_item_managed FROM gl_account ORDER BY code",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn create(
    State(state): State<AppState>,
    Json(account): Json<GlAccount>,
) -> Result<(StatusCode, Json<GlAccount>), AppError> {
    sqlx::query(
        "INSERT INTO gl_account (code, name, kind, currency, open_item_managed) \
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT (code) DO NOTHING",
    )
    .bind(&account.code)
    .bind(&account.name)
    .bind(&account.kind)
    .bind(&account.currency)
    .bind(account.open_item_managed)
    .execute(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(account)))
}
