use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::identity::IdentityType;

/// Immutable, append-only audit record (ADR-009).
///
/// Sensitive values (key_hash, password, JWT signature) MUST NOT appear
/// in any field, including `metadata`. This is enforced at compile time
/// by excluding sensitive fields from Serialize derives in their parent structs.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub id: String,
    pub tenant_id: String,
    /// ID of the User or ServiceAccount that performed the action.
    pub actor_id: String,
    pub actor_type: IdentityType,
    /// Action name, e.g. `"api_key.created"`, `"api_key.revoked"`.
    pub action: String,
    /// Resource type, e.g. `"api_key"`, `"tenant"`.
    pub target_type: String,
    /// Resource ID.
    pub target_id: String,
    /// Additional context (e.g. scopes, reason). NEVER contains key material.
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
