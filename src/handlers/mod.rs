pub mod accounts;
pub mod clearing;
pub mod dunning;
pub mod entries;
pub mod health;
pub mod periods;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/accounts", get(accounts::list).post(accounts::create))
        .route("/entries", post(entries::create))
        .route("/entries/:id", get(entries::get_one))
        .route("/entries/:id/reverse", post(entries::reverse))
        .route("/journal", get(entries::journal))
        .route("/balances", get(entries::balances))
        .route("/open-items", get(clearing::open_items))
        .route("/clearings", post(clearing::create))
        .route("/dunning-runs", post(dunning::run))
        .route("/dunning-runs/:run_id", get(dunning::get_run))
        .route("/periods", get(periods::get).put(periods::put))
        .with_state(state)
}
