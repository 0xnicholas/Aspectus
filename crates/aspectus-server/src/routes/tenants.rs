use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use aspectus_core::{
    audit_log::AuditLog,
    identity::IdentityType,
    store::{AuditLogStore, TenantStore},
};

use crate::error::ProblemDetails;
use crate::AppState;

#[derive(Deserialize)]
pub struct CreateTenantRequest {
    name: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    match state.tenant_store.create(&req.name).await {
        Ok(tenant) => {
            let _ = state.audit_log_store.append(AuditLog {
                id: generate_id(),
                tenant_id: tenant.id.clone(),
                actor_id: "mgmt".into(),
                actor_type: IdentityType::ServiceAccount,
                action: "tenant.created".into(),
                target_type: "tenant".into(),
                target_id: tenant.id.clone(),
                metadata: json!({"name": &req.name}),
                created_at: Utc::now(),
            }).await;

            (StatusCode::CREATED, Json(tenant)).into_response()
        }
        Err(e) => ProblemDetails::from(e).into_response(),
    }
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.tenant_store.get_by_id(&id).await {
        Ok(Some(tenant)) => Json(tenant).into_response(),
        Ok(None) => ProblemDetails::not_found(format!("Tenant {id} not found")).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

fn generate_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("RNG failure");
    hex::encode(&bytes)[..21].to_string()
}
