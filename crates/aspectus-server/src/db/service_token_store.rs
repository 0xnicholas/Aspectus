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
        // Read `project` as text (String) rather than the Project enum directly.
        //
        // Rationale: migration 20260620000013 (KSUID widening) changed
        // `service_tokens.project` from the PostgreSQL `project` enum to
        // `varchar(27)` so that string project names could be stored as KSUID
        // length strings. Direct enum decoding then fails with:
        //
        //   "error occurred while decoding column 0: mismatched types;
        //    Rust type `Project` (as SQL type `project`) is not compatible
        //    with SQL type `VARCHAR`"
        //
        // We parse the string into the enum in Rust instead. The values
        // are still constrained at the database level (CHECK constraint
        // or migration-time validation) — see migration #13.
        let result: Option<(String,)> =
            sqlx::query_as("SELECT project FROM service_tokens WHERE token_hash = $1")
                .bind(token_hash)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| CoreError::Internal(e.to_string()))?;

        Ok(result.and_then(|(s,)| s.parse().ok()))
    }
}
