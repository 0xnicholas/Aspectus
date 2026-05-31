use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub mod api_key_store;
pub mod audit_log_store;
pub mod service_account_store;
pub mod service_token_store;
pub mod tenant_store;

pub use api_key_store::PgApiKeyStore;
pub use audit_log_store::PgAuditLogStore;
pub use service_account_store::PgServiceAccountStore;
pub use service_token_store::PgServiceTokenStore;
pub use tenant_store::PgTenantStore;

/// Initialize a PostgreSQL connection pool and verify connectivity.
pub async fn init_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .connect(database_url)
        .await?;

    sqlx::query("SELECT 1").execute(&pool).await?;
    tracing::info!("PostgreSQL connection pool established (max=20, min=5)");

    Ok(pool)
}
