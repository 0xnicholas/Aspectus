use std::collections::HashMap;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Form, Json};
use serde::Deserialize;

use crate::scope_expander::ScopeExpander;
use crate::AppState;
use aspectus_core::store::TenantStore;

#[derive(Deserialize)]
pub struct IntrospectForm {
    token: String,
    #[serde(default)]
    #[allow(dead_code)]
    token_type_hint: Option<String>,
}

pub async fn handle(
    State(state): State<AppState>,
    Form(form): Form<IntrospectForm>,
) -> impl IntoResponse {
    let mut response = state.api_key_verifier.verify(&form.token).await;

    // v0.8: Record metrics
    crate::routes::metrics::record_introspect(response.active);

    // v0.3.0: Inject tenant quotas into the response
    if response.active {
        // v0.5.0: For User tokens, expand scope from Roles
        if response.identity_type == Some(aspectus_core::identity::IdentityType::User)
            && let Some(ref user_id) = response.user_id {
                response.scope = Some(ScopeExpander::expand(&state.pool, user_id).await);
            }

        if let Some(ref tenant_id) = response.tenant_id
            && let Ok(Some(tenant)) = state.tenant_store.get_by_id(tenant_id).await
                && tenant.quotas != serde_json::Value::Null
                    && tenant.quotas != serde_json::json!({})
                    && let Ok(quotas) =
                        serde_json::from_value::<HashMap<String, serde_json::Value>>(tenant.quotas)
                    {
                        response.quotas = Some(quotas);
                    }
    }

    (StatusCode::OK, Json(response))
}
