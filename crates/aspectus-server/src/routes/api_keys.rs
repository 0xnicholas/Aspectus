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
    project::Project,
    store::{ApiKeyStore, AuditLogStore, ServiceAccountStore},
};

use crate::error::ProblemDetails;
use crate::AppState;

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

    // v0.3.0: Validate scopes exist in the database
    if !req.scopes.is_empty() {
        let valid = sqlx::query_scalar::<_, bool>(
            "SELECT COUNT(*) = $1 FROM scopes WHERE name = ANY($2)",
        )
        .bind(req.scopes.len() as i64)
        .bind(&req.scopes)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(false);

        if !valid {
            return ProblemDetails::validation_failed(
                "One or more scopes are not valid for this project",
                vec![],
            )
            .into_response();
        }
    }

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
            req.scopes.clone(),
            expires_at,
        )
        .await
    {
        Ok(key) => {
            let _ = state.audit_log_store.append(AuditLog {
                id: generate_id(),
                tenant_id: sa.tenant_id,
                actor_id: "mgmt".into(),
                actor_type: IdentityType::ServiceAccount,
                action: "api_key.created".into(),
                target_type: "api_key".into(),
                target_id: key.id.clone(),
                metadata: json!({
                    "service_account_id": &req.service_account_id,
                    "project": req.project,
                    "scopes": &req.scopes,
                }),
                created_at: Utc::now(),
            }).await;

            (StatusCode::CREATED, Json(key)).into_response()
        }
        Err(e) => ProblemDetails::from(e).into_response(),
    }
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.api_key_store.find_by_id(&id).await {
        Ok(Some(key)) => Json(key).into_response(),
        Ok(None) => ProblemDetails::not_found(format!("ApiKey {id} not found")).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
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
    // Look up the key to get its hash for cache invalidation and tenant for audit
    let (key_hash, tenant_id) = match state.api_key_store.find_by_id(&id).await {
        Ok(Some(key)) => (Some(key.key_hash), key.tenant_id),
        _ => (None, String::new()),
    };

    match state.api_key_store.revoke(&id).await {
        Ok(true) => {
            // Invalidate Redis cache so next introspect hits PostgreSQL
            if let Some(hash) = &key_hash {
                state.api_key_verifier.invalidate_cache(hash).await;
            }

            let _ = state.audit_log_store.append(AuditLog {
                id: generate_id(),
                tenant_id: tenant_id.clone(),
                actor_id: "mgmt".into(),
                actor_type: IdentityType::ServiceAccount,
                action: "api_key.revoked".into(),
                target_type: "api_key".into(),
                target_id: id.clone(),
                metadata: json!({"revoked_at": Utc::now().to_rfc3339()}),
                created_at: Utc::now(),
            }).await;

            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => {
            ProblemDetails::not_found(format!("ApiKey {id} not found or already revoked"))
                .into_response()
        }
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

fn generate_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("RNG failure");
    hex::encode(&bytes)[..21].to_string()
}
