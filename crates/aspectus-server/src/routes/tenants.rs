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
    identity::IdentityType,
    store::{AuditLogStore, TenantStore},
};

use crate::AppState;
use crate::error::ProblemDetails;
use crate::util::generate_id;

#[derive(Deserialize)]
pub struct CreateTenantRequest {
    name: String,
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
