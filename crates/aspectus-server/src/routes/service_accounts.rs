use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::error::ProblemDetails;
use crate::AppState;
use aspectus_core::store::ServiceAccountStore;

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
        Ok(sa) => (StatusCode::CREATED, Json(sa)).into_response(),
        Err(e) => ProblemDetails::from(e).into_response(),
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
