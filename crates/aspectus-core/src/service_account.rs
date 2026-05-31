use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Machine identity representing an automated system or CI pipeline.
///
/// Distinguished from `User`: no email/password, scopes bound directly
/// to API Keys in Phase 1, with optional Role support in Phase 2+ (ADR-004).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceAccount {
    pub id: String,
    pub tenant_id: String,
    pub label: String,
    pub description: Option<String>,
    /// Account-level expiry. When reached, all associated API Keys are invalid.
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
