use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Initialize a PostgreSQL connection pool and verify connectivity.
pub async fn init_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .connect(database_url)
        .await?;

    // Verify connection is alive
    sqlx::query("SELECT 1").execute(&pool).await?;
    tracing::info!("PostgreSQL connection pool established (max=20, min=5)");

    Ok(pool)
}
