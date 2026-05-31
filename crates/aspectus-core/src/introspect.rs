use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::identity::IdentityType;

/// RFC 7662 Token Introspection response (ADR-001).
///
/// When `active` is `false`, all other fields serialize as `null`/absent
/// via `skip_serializing_if`. This matches RFC 7662's information-hiding
/// requirement: invalid/expired tokens return `{"active": false}` only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectResponse {
    pub active: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_type: Option<IdentityType>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,

    /// Per-project quota limits (v0.3.0+).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quotas: Option<HashMap<String, serde_json::Value>>,

    /// Token format (v0.4.0+): "api_key" | "jwt" | "opaque"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_format: Option<String>,
}

impl IntrospectResponse {
    /// Convenience constructor for an inactive (invalid/expired/revoked) token.
    pub fn inactive() -> Self {
        Self {
            active: false,
            tenant_id: None,
            user_id: None,
            identity_type: None,
            client_id: None,
            scope: None,
            token_type: None,
            exp: None,
            quotas: None,
            token_format: None,
        }
    }
}
