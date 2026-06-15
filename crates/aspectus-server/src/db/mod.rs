use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub mod api_key_store;
pub mod audit_log_store;
pub mod oauth_store;
pub mod service_account_store;
pub mod service_token_store;
pub mod tenant_store;
pub mod user_store;

pub use api_key_store::PgApiKeyStore;
pub use audit_log_store::PgAuditLogStore;
pub use oauth_store::{PgAuthorizationCodeStore, PgRefreshTokenStore, PgOAuth2ClientStore};
pub use service_account_store::PgServiceAccountStore;
pub use service_token_store::PgServiceTokenStore;
pub use tenant_store::PgTenantStore;
pub use user_store::PgUserStore;

/// Initialize a PostgreSQL connection pool and verify connectivity.
///
/// Pool sizing based on typical Aspectus workload:
/// - max_connections: 50 (handles ~200 concurrent /introspect requests)
/// - min_connections: 10 (avoids cold-start latency)
/// - acquire_timeout: 5s (fast fail vs hanging)
/// - idle_timeout: 300s (release idle connections)
pub async fn init_pool(config: &crate::config::Config) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .min_connections(config.db_min_connections)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .idle_timeout(std::time::Duration::from_secs(300))
        .connect(&config.database_url)
        .await?;

    sqlx::query("SELECT 1").execute(&pool).await?;
    tracing::info!(
        max = config.db_max_connections,
        min = config.db_min_connections,
        "PostgreSQL connection pool established",
    );

    Ok(pool)
}
