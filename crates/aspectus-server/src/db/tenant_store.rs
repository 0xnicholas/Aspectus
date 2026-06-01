use async_trait::async_trait;
use sqlx::PgPool;

use aspectus_core::{error::CoreError, store::TenantStore, tenant::Tenant};

fn generate_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("RNG failure");
    hex::encode(bytes)[..21].to_string()
}

pub struct PgTenantStore {
    pool: PgPool,
}

impl PgTenantStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TenantStore for PgTenantStore {
    async fn create(&self, name: &str) -> Result<Tenant, CoreError> {
        let id = generate_id();

        sqlx::query_as::<_, Tenant>(
            "INSERT INTO tenants (id, name) VALUES ($1, $2) RETURNING *",
        )
        .bind(&id)
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Tenant>, CoreError> {
        sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }
}
