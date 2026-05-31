use async_trait::async_trait;
use sqlx::PgPool;

use aspectus_core::{
    error::CoreError,
    project::Project,
    store::ServiceTokenStore,
};

pub struct PgServiceTokenStore {
    pool: PgPool,
}

impl PgServiceTokenStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ServiceTokenStore for PgServiceTokenStore {
    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Project>, CoreError> {
        let result: Option<(Project,)> =
            sqlx::query_as("SELECT project FROM service_tokens WHERE token_hash = $1")
                .bind(token_hash)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| CoreError::Internal(e.to_string()))?;

        Ok(result.map(|r| r.0))
    }
}
