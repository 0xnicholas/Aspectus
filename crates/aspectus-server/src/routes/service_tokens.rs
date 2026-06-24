//! Service Token management endpoints.
//!
//! These endpoints are protected by the admin service token and allow
//! operators to create, rotate, and revoke the internal tokens that
//! ecosystem projects use to call `POST /introspect`.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use aspectus_auth::{CreatedServiceToken, ServiceTokenCreator};
use aspectus_core::{
    audit_log::AuditLog,
    identity::IdentityType,
    project::Project,
    store::{AuditLogStore, ServiceTokenStore},
};

use crate::AppState;
use crate::error::ProblemDetails;
use crate::util::generate_id;

#[derive(Deserialize)]
pub struct CreateServiceTokenRequest {
    pub project: String,
}

/// Metadata returned by list/get. Never includes the full token or hash.
#[derive(Serialize)]
pub struct ServiceTokenMetadata {
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_prefix: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
}

/// Response when a token is created or rotated. The full `token` is returned
/// exactly once and is never stored or retrievable again.
#[derive(Serialize)]
pub struct ServiceTokenCreatedResponse {
    pub project: String,
    pub token: String,
    pub token_prefix: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&aspectus_core::service_token::ServiceToken> for ServiceTokenMetadata {
    fn from(st: &aspectus_core::service_token::ServiceToken) -> Self {
        Self {
            project: st.project.to_string(),
            token_prefix: st.token_prefix.clone(),
            created_at: st.created_at.to_rfc3339(),
            updated_at: st.updated_at.to_rfc3339(),
            revoked_at: st.revoked_at.map(|dt| dt.to_rfc3339()),
        }
    }
}

impl From<&CreatedServiceToken> for ServiceTokenCreatedResponse {
    fn from(c: &CreatedServiceToken) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            project: c.project.to_string(),
            token: c.token.clone(),
            token_prefix: c.token_prefix.clone(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[allow(clippy::result_large_err)]
fn parse_project(project: &str) -> Result<Project, axum::response::Response> {
    match project.parse() {
        Ok(p) => Ok(p),
        Err(e) => Err(
            ProblemDetails::validation_failed(format!("Invalid project: {e}"), vec![])
                .into_response(),
        ),
    }
}

/// Path-parameter variant: an unknown project string is treated as 404
/// because the resource `/service-tokens/{project}` does not exist.
#[allow(clippy::result_large_err)]
fn parse_project_param(project: &str) -> Result<Project, axum::response::Response> {
    match project.parse() {
        Ok(p) => Ok(p),
        Err(_) => Err(ProblemDetails::not_found(format!(
            "Service token for project {project} not found"
        ))
        .into_response()),
    }
}

#[allow(clippy::result_large_err)]
fn reject_aspectus(project: &Project) -> Result<(), axum::response::Response> {
    if *project == Project::Aspectus {
        return Err(ProblemDetails::with_code(
            aspectus_core::ErrorCode::ValidationFailed,
            "The aspectus admin token cannot be managed through this endpoint",
        )
        .into_response());
    }
    Ok(())
}

async fn append_audit_log(
    audit: &dyn AuditLogStore,
    action: &str,
    project: &Project,
    metadata: serde_json::Value,
) {
    let _ = audit
        .append(AuditLog {
            id: generate_id(),
            tenant_id: "system".into(),
            actor_id: "mgmt".into(),
            actor_type: IdentityType::ServiceAccount,
            action: action.into(),
            target_type: "service_token".into(),
            target_id: project.to_string(),
            metadata,
            created_at: Utc::now(),
        })
        .await;
}

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    match state.service_token_store.list().await {
        Ok(tokens) => {
            let items: Vec<ServiceTokenMetadata> = tokens
                .iter()
                .filter(|t| t.project != Project::Aspectus)
                .map(ServiceTokenMetadata::from)
                .collect();
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list service tokens");
            ProblemDetails::internal_error("Failed to list service tokens").into_response()
        }
    }
}

pub async fn get(State(state): State<AppState>, Path(project): Path<String>) -> impl IntoResponse {
    let project = match parse_project_param(&project) {
        Ok(p) => p,
        Err(pd) => return pd.into_response(),
    };
    if let Err(pd) = reject_aspectus(&project) {
        return pd.into_response();
    }

    match state.service_token_store.get_by_project(&project).await {
        Ok(Some(token)) => {
            (StatusCode::OK, Json(ServiceTokenMetadata::from(&token))).into_response()
        }
        Ok(None) => {
            ProblemDetails::not_found(format!("Service token for project {project} not found"))
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, project = %project, "Failed to get service token");
            ProblemDetails::internal_error("Failed to get service token").into_response()
        }
    }
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateServiceTokenRequest>,
) -> impl IntoResponse {
    let project = match parse_project(&req.project) {
        Ok(p) => p,
        Err(pd) => return pd.into_response(),
    };
    if let Err(pd) = reject_aspectus(&project) {
        return pd.into_response();
    }

    match state.service_token_store.get_by_project(&project).await {
        Ok(Some(existing)) if existing.is_active() => {
            return ProblemDetails::with_code(
                aspectus_core::ErrorCode::Conflict,
                format!("Service token for project {project} already exists. Use /service-tokens/{project}/rotate to replace it."),
            )
            .into_response();
        }
        Ok(_) => {}
        Err(e) => {
            tracing::error!(error = %e, project = %project, "Failed to check existing service token");
            return ProblemDetails::internal_error("Failed to check existing service token")
                .into_response();
        }
    }

    let created = match ServiceTokenCreator::create(project) {
        Ok(c) => c,
        Err(e) => return ProblemDetails::from(e).into_response(),
    };

    match state
        .service_token_store
        .upsert(created.project, &created.token_hash, &created.token_prefix)
        .await
    {
        Ok(_) => {
            append_audit_log(
                state.audit_log_store.as_ref(),
                "service_token.created",
                &created.project,
                json!({"token_prefix": &created.token_prefix}),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(ServiceTokenCreatedResponse::from(&created)),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, project = %project, "Failed to persist service token");
            ProblemDetails::internal_error("Failed to persist service token").into_response()
        }
    }
}

pub async fn rotate(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> impl IntoResponse {
    let project = match parse_project_param(&project) {
        Ok(p) => p,
        Err(pd) => return pd.into_response(),
    };
    if let Err(pd) = reject_aspectus(&project) {
        return pd.into_response();
    }

    let existing = match state.service_token_store.get_by_project(&project).await {
        Ok(Some(t)) if t.is_active() => t,
        Ok(_) => {
            return ProblemDetails::not_found(format!(
                "Active service token for project {project} not found"
            ))
            .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, project = %project, "Failed to look up service token");
            return ProblemDetails::internal_error("Failed to look up service token")
                .into_response();
        }
    };

    let created = match ServiceTokenCreator::create(project) {
        Ok(c) => c,
        Err(e) => return ProblemDetails::from(e).into_response(),
    };

    match state
        .service_token_store
        .upsert(created.project, &created.token_hash, &created.token_prefix)
        .await
    {
        Ok(_) => {
            // Ensure the old token stops working immediately.
            state
                .svc_token_verifier
                .invalidate_by_hash(&existing.token_hash)
                .await;

            append_audit_log(
                state.audit_log_store.as_ref(),
                "service_token.rotated",
                &created.project,
                json!({"token_prefix": &created.token_prefix}),
            )
            .await;
            (
                StatusCode::OK,
                Json(ServiceTokenCreatedResponse::from(&created)),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, project = %project, "Failed to rotate service token");
            ProblemDetails::internal_error("Failed to rotate service token").into_response()
        }
    }
}

pub async fn revoke(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> impl IntoResponse {
    let project = match parse_project_param(&project) {
        Ok(p) => p,
        Err(pd) => return pd.into_response(),
    };
    if let Err(pd) = reject_aspectus(&project) {
        return pd.into_response();
    }

    let existing = match state.service_token_store.get_by_project(&project).await {
        Ok(Some(t)) if t.is_active() => t,
        Ok(_) => {
            return ProblemDetails::not_found(format!(
                "Active service token for project {project} not found"
            ))
            .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, project = %project, "Failed to look up service token");
            return ProblemDetails::internal_error("Failed to look up service token")
                .into_response();
        }
    };

    match state.service_token_store.revoke(&project).await {
        Ok(true) => {
            state
                .svc_token_verifier
                .invalidate_by_hash(&existing.token_hash)
                .await;

            append_audit_log(
                state.audit_log_store.as_ref(),
                "service_token.revoked",
                &project,
                json!({}),
            )
            .await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => ProblemDetails::not_found(format!(
            "Active service token for project {project} not found"
        ))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, project = %project, "Failed to revoke service token");
            ProblemDetails::internal_error("Failed to revoke service token").into_response()
        }
    }
}
