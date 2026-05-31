use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Human user belonging to a Tenant (ADR-004).
///
/// Authenticated via OAuth2 Authorization Code (Phase 3+).
/// Authorized via Role assignment (scope expansion).
///
/// **Security**: `password_hash` is excluded from serialization (`#[serde(skip)]`).
/// It must NEVER appear in API responses, logs, or audit records.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String,
    pub tenant_id: String,
    pub email: Option<String>,
    /// Argon2id hash. `#[serde(skip)]` — never serialized to JSON.
    #[serde(skip)]
    pub password_hash: Option<String>,
    pub display_name: Option<String>,
    pub is_suspended: bool,
    pub last_sign_in_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
