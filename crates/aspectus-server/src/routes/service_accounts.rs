use axum::{
    extract::{Path, Query, State},
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
    store::{AuditLogStore, ServiceAccountStore},
};

use crate::error::ProblemDetails;
use crate::AppState;

#[derive(Deserialize)]
pub struct CreateServiceAccountRequest {
    tenant_id: String,
    label: String,
    description: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    tenant_id: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateServiceAccountRequest>,
) -> impl IntoResponse {
    match state
        .service_account_store
        .create(&req.tenant_id, &req.label, req.description.as_deref())
        .await
    {
        Ok(sa) => {
            let _ = state.audit_log_store.append(AuditLog {
                id: generate_id(),
                tenant_id: sa.tenant_id.clone(),
                actor_id: "mgmt".into(),
                actor_type: IdentityType::ServiceAccount,
                action: "service_account.created".into(),
                target_type: "service_account".into(),
                target_id: sa.id.clone(),
                metadata: json!({"tenant_id": &sa.tenant_id, "label": &sa.label}),
                created_at: Utc::now(),
            }).await;

            (StatusCode::CREATED, Json(sa)).into_response()
        }
        Err(e) => ProblemDetails::from(e).into_response(),
    }
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.service_account_store.get_by_id(&id).await {
        Ok(Some(sa)) => Json(sa).into_response(),
        Ok(None) => ProblemDetails::not_found(format!("ServiceAccount {id} not found")).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    match state
        .service_account_store
        .list_by_tenant(&query.tenant_id)
        .await
    {
        Ok(accounts) => (StatusCode::OK, Json(accounts)).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

fn generate_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("RNG failure");
    hex::encode(&bytes)[..21].to_string()
}
