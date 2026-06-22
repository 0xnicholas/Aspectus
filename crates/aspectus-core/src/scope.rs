use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Permission label in `project:resource:action` format (ADR-006).
///
/// Examples: `pandaria:session:create`, `constell:agent:read`.
/// Supports `*` wildcard for whole-segment matching.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Scope {
    pub id: String,
    /// Full scope string, e.g. `"pandaria:session:create"`.
    pub name: String,
    pub description: Option<String>,
}
