use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use aspectus_core::{project::Project, store::ApiKeyStore};

use crate::error::ProblemDetails;
use crate::AppState;
use aspectus_core::store::ServiceAccountStore;

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    service_account_id: String,
    project: String,
    scopes: Vec<String>,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    service_account_id: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateApiKeyRequest>,
) -> impl IntoResponse {
    let project: Project = match req.project.parse() {
        Ok(p) => p,
        Err(e) => return ProblemDetails::from(e).into_response(),
    };

    // Resolve tenant_id from the service account
    let sa = match state
        .service_account_store
        .get_by_id(&req.service_account_id)
        .await
    {
        Ok(Some(sa)) => sa,
        Ok(None) => {
            return ProblemDetails::not_found(format!(
                "ServiceAccount {} not found",
                req.service_account_id
            ))
            .into_response()
        }
        Err(e) => return ProblemDetails::from(e).into_response(),
    };

    let expires_at = req
        .expires_at
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    match state
        .api_key_creator
        .create(
            &sa.tenant_id,
            &req.service_account_id,
            project,
            req.scopes,
            expires_at,
        )
        .await
    {
        Ok(key) => (StatusCode::CREATED, Json(key)).into_response(),
        Err(e) => ProblemDetails::from(e).into_response(),
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    match state
        .api_key_store
        .list_by_service_account(&query.service_account_id)
        .await
    {
        Ok(keys) => (StatusCode::OK, Json(keys)).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

pub async fn revoke(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.api_key_store.revoke(&id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => {
            ProblemDetails::not_found(format!("ApiKey {id} not found or already revoked"))
                .into_response()
        }
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}
