use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use aspectus_core::{
    audit_log::AuditLog,
    identity::IdentityType,
    project::Project,
    scope::Scope,
    store::{ApiKeyStore, AuditLogStore, ServiceAccountStore, UserStore},
};

use crate::AppState;
use crate::error::ProblemDetails;
use crate::util::generate_id;

const MAX_SCOPES_PER_KEY: usize = Scope::MAX_SCOPES_PER_KEY;

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    #[serde(default)]
    owner_type: Option<String>,
    #[serde(default)]
    owner_id: Option<String>,
    #[serde(default)]
    service_account_id: Option<String>,
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

    // Resolve owner: new format takes priority, fallback to service_account_id
    let owner_type = req.owner_type.as_deref().unwrap_or("service_account");
    let owner_id = if let Some(id) = &req.owner_id {
        id.clone()
    } else if let Some(sa_id) = &req.service_account_id {
        sa_id.clone()
    } else {
        return ProblemDetails::validation_failed(
            "Either owner_type+owner_id or service_account_id is required",
            vec![],
        )
        .into_response();
    };

    if !["user", "service_account"].contains(&owner_type) {
        return ProblemDetails::validation_failed(
            format!("Invalid owner_type: {owner_type}"),
            vec![],
        )
        .into_response();
    }

    // v0.9.0: Scope count limit
    if req.scopes.len() > MAX_SCOPES_PER_KEY {
        return ProblemDetails::validation_failed(
            format!("Too many scopes: max {MAX_SCOPES_PER_KEY}"),
            vec![],
        )
        .into_response();
    }

    // Validate each scope is syntactically valid and belongs to the key's project.
    for scope in &req.scopes {
        if let Err(e) = Scope::validate(scope) {
            return ProblemDetails::validation_failed(
                format!("Invalid scope '{scope}': {e}"),
                vec![],
            )
            .into_response();
        }
        match Scope::project(scope) {
            Ok(scope_project) if scope_project == project => {}
            Ok(scope_project) => {
                return ProblemDetails::validation_failed(
                    format!(
                        "Scope '{scope}' belongs to project '{scope_project}' but key project is '{}'",
                        project
                    ),
                    vec![],
                )
                .into_response();
            }
            Err(e) => {
                return ProblemDetails::validation_failed(
                    format!("Invalid scope '{scope}': {e}"),
                    vec![],
                )
                .into_response();
            }
        }
    }

    // v0.3.0: Validate scopes exist in the database
    if !req.scopes.is_empty() {
        let valid = match sqlx::query_scalar::<_, bool>(
            "SELECT COUNT(*) = $1 FROM scopes WHERE name = ANY($2)",
        )
        .bind(req.scopes.len() as i64)
        .bind(&req.scopes)
        .fetch_one(&state.pool)
        .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "Scope existence check failed");
                return ProblemDetails::internal_error("Failed to validate scopes").into_response();
            }
        };

        if !valid {
            return ProblemDetails::validation_failed(
                "One or more scopes are not valid for this project",
                vec![],
            )
            .into_response();
        }
    }

    // Resolve tenant_id from owner
    let tenant_id = match owner_type {
        "user" => match state.user_store.get_by_id(&owner_id).await {
            Ok(Some(user)) => user.tenant_id,
            Ok(None) => {
                return ProblemDetails::not_found(format!("User {owner_id} not found"))
                    .into_response();
            }
            Err(e) => return ProblemDetails::from(e).into_response(),
        },
        _ => match state.service_account_store.get_by_id(&owner_id).await {
            Ok(Some(sa)) => sa.tenant_id,
            Ok(None) => {
                return ProblemDetails::not_found(format!("ServiceAccount {owner_id} not found"))
                    .into_response();
            }
            Err(e) => return ProblemDetails::from(e).into_response(),
        },
    };

    let expires_at = req
        .expires_at
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    // For v0.5, pass owner_id as service_account_id (creator handles both)
    match state
        .api_key_creator
        .create(
            &tenant_id,
            &owner_id,
            project,
            req.scopes.clone(),
            expires_at,
        )
        .await
    {
        Ok(key) => {
            // v0.7: Set user_id for user-owned keys (creator writes to service_account_id by default)
            if owner_type == "user" {
                let _ = sqlx::query(
                    "UPDATE api_keys SET user_id = $1, service_account_id = NULL WHERE id = $2",
                )
                .bind(&owner_id)
                .bind(&key.id)
                .execute(&state.pool)
                .await;
            }

            let _ = state
                .audit_log_store
                .append(AuditLog {
                    id: generate_id(),
                    tenant_id: tenant_id.clone(),
                    actor_id: "mgmt".into(),
                    actor_type: IdentityType::ServiceAccount,
                    action: "api_key.created".into(),
                    target_type: "api_key".into(),
                    target_id: key.id.clone(),
                    metadata: json!({
                        "owner_type": owner_type, "owner_id": &owner_id,
                        "project": req.project,
                        "scopes": &req.scopes,
                    }),
                    created_at: Utc::now(),
                })
                .await;

            (StatusCode::CREATED, Json(key)).into_response()
        }
        Err(e) => ProblemDetails::from(e).into_response(),
    }
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.api_key_store.find_by_id(&id).await {
        Ok(Some(key)) => {
            let item = aspectus_core::api_key::ApiKeyListItem {
                id: key.id,
                service_account_id: key.service_account_id,
                project: key.project,
                key_prefix: key.key_prefix,
                scopes: key.scopes,
                expires_at: key.expires_at,
                revoked_at: key.revoked_at,
                created_at: key.created_at,
            };
            Json(item).into_response()
        }
        Ok(None) => ProblemDetails::not_found(format!("ApiKey {id} not found")).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get API key");
            ProblemDetails::internal_error("An internal error occurred").into_response()
        }
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

pub async fn revoke(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    // Look up the key for cache invalidation and tenant for audit
    let key = match state.api_key_store.find_by_id(&id).await {
        Ok(Some(key)) => key,
        _ => return ProblemDetails::not_found(format!("ApiKey {id} not found")).into_response(),
    };

    match state.api_key_store.revoke(&id).await {
        Ok(true) => {
            // Invalidate Redis cache so next introspect hits PostgreSQL
            state.api_key_verifier.invalidate_cache(&key.key_hash).await;

            let _ = state
                .audit_log_store
                .append(AuditLog {
                    id: generate_id(),
                    tenant_id: key.tenant_id.clone(),
                    actor_id: "mgmt".into(),
                    actor_type: IdentityType::ServiceAccount,
                    action: "api_key.revoked".into(),
                    target_type: "api_key".into(),
                    target_id: id.clone(),
                    metadata: json!({"revoked_at": Utc::now().to_rfc3339()}),
                    created_at: Utc::now(),
                })
                .await;

            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => ProblemDetails::not_found(format!("ApiKey {id} not found or already revoked"))
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to revoke API key");
            ProblemDetails::internal_error("An internal error occurred").into_response()
        }
    }
}
