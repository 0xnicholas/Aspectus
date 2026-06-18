use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use aspectus_core::identity::IdentityType;
use aspectus_core::project::Project;

use crate::error::ProblemDetails;
use crate::AppState;
use aspectus_core::store::ApiKeyStore;

#[derive(Deserialize)]
pub struct TokenRequest {
    grant_type: String,
    client_id: String,
    client_secret: String,
    #[serde(default = "default_token_format")]
    token_format: String,
}

fn default_token_format() -> String {
    "jwt".into()
}

pub async fn issue(
    State(state): State<AppState>,
    Json(req): Json<TokenRequest>,
) -> impl IntoResponse {
    if req.grant_type != "client_credentials" {
        return ProblemDetails::validation_failed("Only client_credentials is supported", vec![])
            .into_response();
    }

    let introspect = state.api_key_verifier.verify(&req.client_secret).await;
    if !introspect.active {
        return ProblemDetails::unauthorized("Invalid client_secret", "/token").into_response();
    }

    let tenant_id = introspect.tenant_id.unwrap_or_default();
    let scopes = introspect.scope.unwrap_or_default();
    let project: Project = introspect
        .client_id
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Project::Pandaria);

    match req.token_format.as_str() {
        "jwt" => {
            let ttl: u64 = std::env::var("JWT_TTL_SECONDS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(900);
            match state.jwt_signer.sign(&req.client_id, &tenant_id, project, &scopes, IdentityType::ServiceAccount, ttl) {
                Ok(token) => (StatusCode::OK, Json(json!({
                    "access_token": token, "token_format": "jwt",
                    "expires_in": ttl, "token_type": "Bearer"
                }))).into_response(),
                Err(e) => ProblemDetails::from(e).into_response(),
            }
        }
        "opaque" => {
            let ttl: u64 = std::env::var("OPAQUE_TOKEN_TTL_SECONDS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(3600);
            match state.api_key_creator.create_opaque(
                &tenant_id, &req.client_id, project, &scopes, ttl,
            ).await {
                Ok(token) => (StatusCode::OK, Json(json!({
                    "access_token": token.key, "token_format": "opaque",
                    "expires_in": ttl, "token_type": "Bearer"
                }))).into_response(),
                Err(e) => ProblemDetails::from(e).into_response(),
            }
        }
        other => ProblemDetails::validation_failed(
            format!("Unsupported token_format: {other}"), vec![],
        ).into_response(),
    }
}

pub async fn revoke(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let token = body.get("token").and_then(|v| v.as_str()).unwrap_or("");
    if token.is_empty() {
        return ProblemDetails::validation_failed("token is required", vec![]).into_response();
    }

    if token.starts_with("eyJ") {
        state.jwt_verifier.revoke(token).await;
    } else {
        // API Key or Opaque: extract raw bytes, hash, find, revoke
        let raw = token
            .strip_prefix("pk_live_")
            .or_else(|| token.strip_prefix("ot_"))
            .and_then(|s| hex::decode(s).ok());
        if let Some(raw_bytes) = raw {
            let hash = hex::encode(Sha256::digest(&raw_bytes));
            if let Ok(Some(key)) = state.api_key_store.find_by_hash(&hash).await {
                let _ = state.api_key_store.revoke(&key.id).await;
            }
        }
    }

    StatusCode::NO_CONTENT.into_response()
}

pub async fn jwks(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(state.jwt_signer.jwks_json())
}
