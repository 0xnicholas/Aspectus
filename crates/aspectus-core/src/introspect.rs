use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{error::CoreError, identity::IdentityType};

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

    /// Constructor for an active introspection response.
    ///
    /// Requires the fields that RFC 7662 consumers rely on: `tenant_id`,
    /// `scope`, and `exp`. Use [`IntrospectResponse::inactive`] for invalid tokens.
    #[allow(clippy::too_many_arguments)]
    pub fn active(
        tenant_id: impl Into<String>,
        user_id: impl Into<String>,
        identity_type: IdentityType,
        client_id: impl Into<String>,
        scope: impl Into<String>,
        exp: i64,
        quotas: Option<std::collections::HashMap<String, serde_json::Value>>,
        token_format: impl Into<String>,
    ) -> Self {
        Self {
            active: true,
            tenant_id: Some(tenant_id.into()),
            user_id: Some(user_id.into()),
            identity_type: Some(identity_type),
            client_id: Some(client_id.into()),
            scope: Some(scope.into()),
            token_type: Some("Bearer".into()),
            exp: Some(exp),
            quotas,
            token_format: Some(token_format.into()),
        }
    }

    /// Validate that an active response carries the mandatory fields.
    ///
    /// Returns `Ok(())` for inactive responses regardless of other fields.
    pub fn validate(&self) -> Result<(), CoreError> {
        if !self.active {
            return Ok(());
        }
        if self.tenant_id.is_none() {
            return Err(CoreError::Validation(
                "active introspection response missing tenant_id".into(),
            ));
        }
        if self.scope.is_none() {
            return Err(CoreError::Validation(
                "active introspection response missing scope".into(),
            ));
        }
        if self.exp.is_none() {
            return Err(CoreError::Validation(
                "active introspection response missing exp".into(),
            ));
        }
        Ok(())
    }
}
