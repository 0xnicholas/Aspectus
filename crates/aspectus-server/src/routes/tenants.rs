use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use aspectus_core::{
    audit_log::AuditLog,
    error::CoreError,
    identity::IdentityType,
    project::Project,
    store::{AuditLogStore, TenantStore},
};

use crate::AppState;
use crate::error::ProblemDetails;
use crate::util::generate_id;

#[derive(Deserialize)]
pub struct CreateTenantRequest {
    name: String,
}

/// Known numeric quota keys that must be non-negative integers if present.
const NUMERIC_QUOTA_KEYS: &[&str] = &[
    "monthly_tokens",
    "max_concurrent_sessions",
    "max_users",
    "max_api_keys",
    "requests_per_minute",
    "monthly_requests",
];

/// Validate that tenant quotas are a JSON object whose top-level keys are
/// known ecosystem projects. Per-project values must also be objects, and any
/// recognized numeric quota keys must hold non-negative integer values.
fn validate_quotas(quotas: &serde_json::Value) -> Result<(), CoreError> {
    let obj = quotas
        .as_object()
        .ok_or_else(|| CoreError::Validation("quotas must be a JSON object".into()))?;
    for (project, project_value) in obj {
        if project.parse::<Project>().is_err() {
            return Err(CoreError::Validation(format!(
                "quota key '{project}' is not a known project"
            )));
        }
        let project_obj = project_value.as_object().ok_or_else(|| {
            CoreError::Validation(format!(
                "quota for project '{project}' must be a JSON object"
            ))
        })?;
        for (key, value) in project_obj {
            if NUMERIC_QUOTA_KEYS.contains(&key.as_str()) && value.as_u64().is_none() {
                return Err(CoreError::Validation(format!(
                    "quota '{project}.{key}' must be a non-negative integer"
                )));
            }
        }
    }
    Ok(())
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    // Validate tenant name: non-empty, ≤128 chars, [a-zA-Z0-9_-] only
    if req.name.is_empty() || req.name.len() > 128 {
        return ProblemDetails::validation_failed(
            "Tenant name must be between 1 and 128 characters",
            vec![],
        )
        .into_response();
    }
    if !req
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return ProblemDetails::validation_failed(
            "Tenant name may only contain letters, numbers, underscore, and hyphen",
            vec![],
        )
        .into_response();
    }

    match state.tenant_store.create(&req.name).await {
        Ok(tenant) => {
            let _ = state
                .audit_log_store
                .append(AuditLog {
                    id: generate_id(),
                    tenant_id: tenant.id.clone(),
                    actor_id: "mgmt".into(),
                    actor_type: IdentityType::ServiceAccount,
                    action: "tenant.created".into(),
                    target_type: "tenant".into(),
                    target_id: tenant.id.clone(),
                    metadata: json!({"name": &req.name}),
                    created_at: Utc::now(),
                })
                .await;

            (StatusCode::CREATED, Json(tenant)).into_response()
        }
        Err(e) => ProblemDetails::from(e).into_response(),
    }
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.tenant_store.get_by_id(&id).await {
        Ok(Some(tenant)) => Json(tenant).into_response(),
        Ok(None) => ProblemDetails::not_found(format!("Tenant {id} not found")).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    match state.tenant_store.list().await {
        Ok(tenants) => (StatusCode::OK, Json(tenants)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to list tenants");
            ProblemDetails::internal_error("Failed to list tenants").into_response()
        }
    }
}

pub async fn update_quotas(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(quotas): Json<serde_json::Value>,
) -> impl IntoResponse {
    if let Err(e) = validate_quotas(&quotas) {
        return ProblemDetails::from(e).into_response();
    }

    match sqlx::query("UPDATE tenants SET quotas = $1 WHERE id = $2")
        .bind(&quotas)
        .bind(&id)
        .execute(&state.pool)
        .await
    {
        Ok(result) if result.rows_affected() > 0 => {
            let _ = state
                .audit_log_store
                .append(AuditLog {
                    id: generate_id(),
                    tenant_id: id.clone(),
                    actor_id: "mgmt".into(),
                    actor_type: IdentityType::ServiceAccount,
                    action: "quota.updated".into(),
                    target_type: "tenant".into(),
                    target_id: id,
                    metadata: json!({"quotas": &quotas}),
                    created_at: Utc::now(),
                })
                .await;

            StatusCode::NO_CONTENT.into_response()
        }
        Ok(_) => ProblemDetails::not_found("Tenant not found").into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotas_must_be_object() {
        let v = serde_json::json!("not an object");
        assert!(validate_quotas(&v).is_err());
    }

    #[test]
    fn quotas_reject_unknown_project() {
        let v = serde_json::json!({"unknown": {}});
        assert!(validate_quotas(&v).is_err());
    }

    #[test]
    fn quotas_require_project_value_to_be_object() {
        let v = serde_json::json!({"pandaria": "not an object"});
        assert!(validate_quotas(&v).is_err());
    }

    #[test]
    fn quotas_accept_empty_object() {
        assert!(validate_quotas(&serde_json::json!({})).is_ok());
    }

    #[test]
    fn quotas_accept_known_project_with_unknown_keys() {
        let v = serde_json::json!({"pandaria": {"custom_setting": true}});
        assert!(validate_quotas(&v).is_ok());
    }

    #[test]
    fn quotas_reject_negative_numeric_key() {
        let v = serde_json::json!({"pandaria": {"monthly_tokens": -1}});
        assert!(validate_quotas(&v).is_err());
    }

    #[test]
    fn quotas_reject_float_for_numeric_key() {
        let v = serde_json::json!({"pandaria": {"monthly_tokens": 1.5}});
        assert!(validate_quotas(&v).is_err());
    }

    #[test]
    fn quotas_accept_zero_for_numeric_key() {
        let v = serde_json::json!({"pandaria": {"monthly_tokens": 0}});
        assert!(validate_quotas(&v).is_ok());
    }

    #[test]
    fn quotas_accept_valid_numeric_key() {
        let v = serde_json::json!({"tokencamp": {"monthly_tokens": 1000000}});
        assert!(validate_quotas(&v).is_ok());
    }
}
