use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::error::ProblemDetails;
use crate::AppState;
use aspectus_core::store::TenantStore;

#[derive(Deserialize)]
pub struct CreateTenantRequest {
    name: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    match state.tenant_store.create(&req.name).await {
        Ok(tenant) => (StatusCode::CREATED, Json(tenant)).into_response(),
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
