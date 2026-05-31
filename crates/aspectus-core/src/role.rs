use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::identity::RoleType;

/// Named collection of scopes, globally defined (not per-tenant).
///
/// Each Role is tagged with a `RoleType` that constrains which identity
/// types (User / ServiceAccount) it can be assigned to (ADR-005).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub id: String,
    /// Unique role name, e.g. `"agent-developer"`.
    pub name: String,
    pub description: Option<String>,
    /// Constrains assignment: `user`, `service_account`, or `both`.
    pub r#type: RoleType,
    /// If `true`, this role is automatically assigned to new users.
    pub is_default: bool,
}
