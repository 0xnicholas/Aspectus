use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Top-level namespace for the Pandaria ecosystem.
///
/// Every resource (User, ServiceAccount, ApiKey, AuditLog) is scoped
/// to a single tenant. Cross-tenant operations are inexpressible by design (ADR-008).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    /// Per-project quota limits (v0.3.0+). Stored as JSONB.
    pub quotas: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
