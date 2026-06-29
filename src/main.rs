mod clearing;
mod db;
mod dunning;
mod error;
mod handlers;
mod ledger;
mod model;

use std::env;

use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Shared application state: just the database pool.
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().compact())
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .init();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://core:core@localhost:5435/modern_core".to_string());
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8091);

    tracing::info!("🏦 Starting nano-bank modern core");
    let pool = db::connect(&database_url).await?;
    db::bootstrap(&pool).await?;
    tracing::info!("✅ schema and seed applied");

    let app = handlers::router(AppState { pool }).layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("🚀 modern-core listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
