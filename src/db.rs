use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Open a connection pool to Postgres.
pub async fn connect(url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(url)
        .await?;
    Ok(pool)
}

/// Apply the schema and seed at startup. Both files are idempotent
/// (`CREATE TABLE IF NOT EXISTS` / `INSERT ... ON CONFLICT DO NOTHING`), so this
/// is safe to run on every boot. Statements are split on `;` after stripping
/// full-line `--` comments; the SQL files deliberately avoid inline comments and
/// embedded semicolons so this stays simple.
pub async fn bootstrap(pool: &PgPool) -> anyhow::Result<()> {
    for file in [
        include_str!("../resources/schema.sql"),
        include_str!("../resources/seed.sql"),
    ] {
        let cleaned: String = file
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        for statement in cleaned.split(';') {
            let stmt = statement.trim();
            if stmt.is_empty() {
                continue;
            }
            sqlx::query(stmt).execute(pool).await?;
        }
    }
    Ok(())
}
