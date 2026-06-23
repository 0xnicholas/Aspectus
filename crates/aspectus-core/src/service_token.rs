//! Service Token domain model.
//!
//! Service tokens are the internal credentials that ecosystem projects
//! (Pandaria, Constell, Tokencamp, Emerald, Heirloom) use to authenticate
//! calls to `POST /introspect`. Each consumer project has at most one active
//! token at a time (ADR-011).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::project::Project;

/// Stored metadata for a project service token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceToken {
    pub project: Project,
    pub token_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_prefix: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<DateTime<Utc>>,
}

impl ServiceToken {
    /// Whether the token has not been soft-revoked.
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }
}
