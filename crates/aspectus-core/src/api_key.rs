use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{error::CoreError, project::Project, scope::Scope};

/// Long-lived credential scoped to a single (tenant, project, scopes) tuple.
///
/// Stored as `sha256(key)` in the database. The raw key is returned exactly once
/// at creation time. Lost keys must be re-created (ADR-002).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: String,
    pub tenant_id: String,
    pub service_account_id: String,
    pub project: Project,
    /// `sha256(raw_key)` — never exposed via API responses.
    pub key_hash: String,
    /// Display prefix, e.g. `pk_live_aBcDeFgH`.
    pub key_prefix: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    /// `None` = active, `Some` = revoked at this timestamp.
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl ApiKey {
    /// Validate that all attached scopes are well-formed and belong to this key's project.
    pub fn validate_scopes(&self) -> Result<(), CoreError> {
        if self.scopes.len() > Scope::MAX_SCOPES_PER_KEY {
            return Err(CoreError::Validation(format!(
                "too many scopes: {} exceeds limit {}",
                self.scopes.len(),
                Scope::MAX_SCOPES_PER_KEY
            )));
        }
        for scope in &self.scopes {
            Scope::validate(scope)
                .map_err(|e| CoreError::Validation(format!("invalid scope '{scope}': {e}")))?;
            let scope_project = Scope::project(scope)
                .map_err(|e| CoreError::Validation(format!("invalid scope '{scope}': {e}")))?;
            if scope_project != self.project {
                return Err(CoreError::Validation(format!(
                    "scope '{scope}' belongs to project '{scope_project}' but key project is '{}'",
                    self.project
                )));
            }
        }
        Ok(())
    }
}

/// Returned to the caller once when an API Key is created.
///
/// Contains the raw key. This is the ONLY time the raw key is exposed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedApiKey {
    pub id: String,
    /// Raw key in `pk_live_` format. Store it safely — it won't be shown again.
    pub key: String,
    pub key_prefix: String,
    pub project: Project,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Lightweight representation for list endpoints.
///
/// Does NOT include `key_hash` or raw key.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKeyListItem {
    pub id: String,
    pub service_account_id: String,
    pub project: Project,
    pub key_prefix: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
