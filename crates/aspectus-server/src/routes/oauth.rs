use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use aspectus_auth::password::PasswordHasher;
use aspectus_core::identity::IdentityType;
use aspectus_core::store::{AuthorizationCodeStore, OAuth2ClientStore, RefreshTokenStore};

use crate::AppState;
use crate::error::ProblemDetails;
use crate::util::generate_id;

// ---- /authorize ----

#[derive(Deserialize)]
pub struct AuthorizeRequest {
    email: String,
    password: String,
    /// ADR-016: tenant_id is REQUIRED to disambiguate when the same email
    /// is registered under multiple tenants (schema UNIQUE (tenant_id, email)).
    tenant_id: String,
    client_id: String,
    redirect_uri: String,
    #[serde(default)]
    code_challenge: Option<String>,
    #[serde(default)]
    code_challenge_method: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    state: Option<String>,
}

pub async fn authorize(
    State(state): State<AppState>,
    Json(req): Json<AuthorizeRequest>,
) -> impl IntoResponse {
    // Validate redirect_uri against registered client
    let valid = match state
        .oauth_client_store
        .validate_redirect_uri(&req.client_id, &req.redirect_uri)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "Failed to validate OAuth2 client");
            return ProblemDetails::internal_error(
                "Authentication service temporarily unavailable",
            )
            .into_response();
        }
    };

    if !valid {
        return ProblemDetails::validation_failed("Invalid client_id or redirect_uri", vec![])
            .into_response();
    }

    // PKCE: if code_challenge is provided, method must be S256
    if req.code_challenge.is_some() && req.code_challenge_method.as_deref() != Some("S256") {
        return ProblemDetails::validation_failed(
            "code_challenge_method must be S256 when code_challenge is provided",
            vec![],
        )
        .into_response();
    }

    // ADR-016: look up user by (tenant_id, email) to prevent cross-tenant
    // email collisions. The same email can exist in multiple tenants.
    let (user_id, _tenant_id) = match sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, tenant_id, password_hash FROM users WHERE tenant_id = $1 AND email = $2",
    )
    .bind(&req.tenant_id)
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some((id, tid, hash))) => match PasswordHasher::verify(&req.password, &hash) {
            Ok(true) => (id, tid),
            _ => {
                return ProblemDetails::unauthorized("Invalid credentials", "/authorize")
                    .into_response();
            }
        },
        Ok(None) => {
            return ProblemDetails::unauthorized("Invalid credentials", "/authorize")
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, tenant_id = %req.tenant_id, email_hash = %hex::encode(Sha256::digest(req.email.as_bytes())), "User lookup failed");
            return ProblemDetails::internal_error(
                "Authentication service temporarily unavailable",
            )
            .into_response();
        }
    };

    // Check if user is suspended before issuing any code.
    let is_suspended: bool = match sqlx::query_scalar(
        "SELECT is_suspended FROM users WHERE id = $1",
    )
    .bind(&user_id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, user_id = %user_id, "Suspension check failed during authorize");
            return ProblemDetails::internal_error(
                "Authentication service temporarily unavailable",
            )
            .into_response();
        }
    };
    if is_suspended {
        return ProblemDetails::unauthorized("Account is suspended", "/authorize").into_response();
    }

    // Generate authorization code
    let mut raw = [0u8; 32];
    if let Err(e) = getrandom::getrandom(&mut raw) {
        tracing::error!(error = %e, "Failed to generate authorization code");
        return ProblemDetails::internal_error("Authentication service temporarily unavailable")
            .into_response();
    }
    let code = hex::encode(Sha256::digest(raw));

    let expires_at = Utc::now() + chrono::Duration::seconds(300);

    if let Err(e) = state
        .auth_code_store
        .create_code(
            &code,
            &user_id,
            &req.client_id,
            &req.redirect_uri,
            expires_at,
        )
        .await
    {
        tracing::error!(error = %e, "Failed to store authorization code");
        return ProblemDetails::internal_error("Authentication service temporarily unavailable")
            .into_response();
    }

    // Store code_challenge if PKCE is in use
    if let Some(ref challenge) = req.code_challenge {
        let _ = sqlx::query("UPDATE authorization_codes SET code_challenge = $1 WHERE code = $2")
            .bind(challenge)
            .bind(&code)
            .execute(&state.pool)
            .await;
    }

    (
        StatusCode::OK,
        Json(json!({"code": code, "redirect_uri": req.redirect_uri})),
    )
        .into_response()
}

// ---- /token ----

#[derive(Deserialize)]
pub struct TokenRequest {
    grant_type: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    code_verifier: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    client_secret: Option<String>,
    #[serde(default)]
    redirect_uri: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

pub async fn token(
    State(state): State<AppState>,
    Json(req): Json<TokenRequest>,
) -> impl IntoResponse {
    match req.grant_type.as_str() {
        "authorization_code" => {
            let code = match &req.code {
                Some(c) => c.clone(),
                None => {
                    return ProblemDetails::validation_failed("code required", vec![])
                        .into_response();
                }
            };

            let request_client_id = match &req.client_id {
                Some(id) => id.as_str(),
                None => {
                    return ProblemDetails::validation_failed("client_id required", vec![])
                        .into_response();
                }
            };
            let client_secret = req.client_secret.as_deref().unwrap_or("");

            match state
                .oauth_client_store
                .validate_client_secret(request_client_id, client_secret)
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    return ProblemDetails::unauthorized(
                        "Invalid client_id or client_secret",
                        "/token",
                    )
                    .into_response();
                }
                Err(e) => {
                    tracing::error!(error = %e, "Client secret validation failed");
                    return ProblemDetails::internal_error("Token service temporarily unavailable")
                        .into_response();
                }
            }

            let row = match state.auth_code_store.exchange_code(&code).await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    return ProblemDetails::unauthorized("Invalid or expired code", "/token")
                        .into_response();
                }
                Err(e) => {
                    tracing::error!(error = %e, "Code exchange failed");
                    return ProblemDetails::internal_error("Token service temporarily unavailable")
                        .into_response();
                }
            };
            let (user_id, client_id, stored_redirect_uri) = row;

            if client_id != request_client_id {
                return ProblemDetails::unauthorized("Invalid client_id", "/token").into_response();
            }

            // OAuth 2.0: redirect_uri must match the one used in the authorize
            // request exactly. This prevents authorization code interception attacks.
            let request_redirect_uri = req.redirect_uri.as_deref().unwrap_or("");
            if request_redirect_uri.is_empty() {
                return ProblemDetails::validation_failed("redirect_uri required", vec![])
                    .into_response();
            }
            if request_redirect_uri != stored_redirect_uri {
                return ProblemDetails::unauthorized("redirect_uri mismatch", "/token")
                    .into_response();
            }

            // PKCE: enforce code_verifier when a code_challenge was stored.
            // If the authorization request included a challenge, the token request
            // MUST include a matching verifier (OAuth 2.1 / RFC 7636).
            let challenge: Option<String> = match sqlx::query_scalar(
                "SELECT code_challenge FROM authorization_codes WHERE code = $1",
            )
            .bind(&code)
            .fetch_optional(&state.pool)
            .await
            {
                Ok(v) => v.flatten(),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to read stored PKCE challenge");
                    return ProblemDetails::internal_error("Token service temporarily unavailable")
                        .into_response();
                }
            };

            match (challenge, req.code_verifier.as_ref()) {
                (Some(expected_challenge), Some(verifier)) => {
                    let actual_challenge = hex::encode(Sha256::digest(verifier.as_bytes()));
                    if !constant_time_eq_hex(&expected_challenge, &actual_challenge) {
                        return ProblemDetails::unauthorized("Invalid code_verifier", "/token")
                            .into_response();
                    }
                }
                (Some(_), None) => {
                    return ProblemDetails::unauthorized(
                        "code_verifier required (PKCE challenge was sent)",
                        "/token",
                    )
                    .into_response();
                }
                (None, Some(_)) => {
                    return ProblemDetails::unauthorized(
                        "code_verifier provided but no PKCE challenge was stored",
                        "/token",
                    )
                    .into_response();
                }
                (None, None) => {}
            }

            let tenant_id =
                match sqlx::query_as::<_, (String,)>("SELECT tenant_id FROM users WHERE id = $1")
                    .bind(&user_id)
                    .fetch_optional(&state.pool)
                    .await
                {
                    Ok(Some((tid,))) => tid,
                    Ok(None) => {
                        return ProblemDetails::internal_error("User not found").into_response();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, user_id = %user_id, "User lookup failed");
                        return ProblemDetails::internal_error(
                            "Token service temporarily unavailable",
                        )
                        .into_response();
                    }
                };

            issue_tokens(&state, &user_id, &tenant_id, &client_id)
                .await
                .into_response()
        }
        "refresh_token" => {
            let token = match &req.refresh_token {
                Some(t) => t.clone(),
                None => {
                    return ProblemDetails::validation_failed("refresh_token required", vec![])
                        .into_response();
                }
            };

            let request_client_id = match &req.client_id {
                Some(id) => id.as_str(),
                None => {
                    return ProblemDetails::validation_failed("client_id required", vec![])
                        .into_response();
                }
            };
            let client_secret = req.client_secret.as_deref().unwrap_or("");

            match state
                .oauth_client_store
                .validate_client_secret(request_client_id, client_secret)
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    return ProblemDetails::unauthorized(
                        "Invalid client_id or client_secret",
                        "/token",
                    )
                    .into_response();
                }
                Err(e) => {
                    tracing::error!(error = %e, "Client secret validation failed");
                    return ProblemDetails::internal_error("Token service temporarily unavailable")
                        .into_response();
                }
            }

            let hash = hex::encode(Sha256::digest(token.as_bytes()));

            match state.refresh_token_store.rotate(&hash).await {
                Ok(Some((user_id, client_id, _))) => {
                    if client_id != request_client_id {
                        return ProblemDetails::unauthorized("Invalid client_id", "/token")
                            .into_response();
                    }
                    let (tenant_id, is_suspended) = match sqlx::query_as::<_, (String, bool)>(
                        "SELECT tenant_id, is_suspended FROM users WHERE id = $1",
                    )
                    .bind(&user_id)
                    .fetch_optional(&state.pool)
                    .await
                    {
                        Ok(Some((tid, suspended))) => (tid, suspended),
                        Ok(None) => {
                            return ProblemDetails::internal_error("User not found")
                                .into_response();
                        }
                        Err(e) => {
                            tracing::error!(error = %e, user_id = %user_id, "User lookup failed during refresh");
                            return ProblemDetails::internal_error(
                                "Token service temporarily unavailable",
                            )
                            .into_response();
                        }
                    };
                    if is_suspended {
                        return ProblemDetails::unauthorized("Account is suspended", "/token")
                            .into_response();
                    }
                    issue_tokens(&state, &user_id, &tenant_id, &client_id)
                        .await
                        .into_response()
                }
                Ok(None) => {
                    // Token not found or already revoked — check for replay attack
                    if let Ok(Some((user_id, is_revoked))) =
                        state.refresh_token_store.find_by_hash_any(&hash).await
                        && is_revoked
                    {
                        // Replay detected: revoke all tokens for this user
                        let count = state
                            .refresh_token_store
                            .revoke_all_for_user(&user_id)
                            .await
                            .unwrap_or(0);
                        tracing::warn!(
                            user_id = %user_id,
                            revoked = count,
                            "Refresh token replay detected — revoked all user tokens"
                        );
                    }
                    ProblemDetails::unauthorized("Invalid or expired refresh_token", "/token")
                        .into_response()
                }
                Err(e) => {
                    tracing::error!(error = %e, "Refresh token rotation failed");
                    ProblemDetails::internal_error("Token service temporarily unavailable")
                        .into_response()
                }
            }
        }
        _ => ProblemDetails::validation_failed("Unsupported grant_type", vec![]).into_response(),
    }
}

/// Constant-time comparison of two equal-length lowercase hex strings.
fn constant_time_eq_hex(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

pub async fn issue_tokens(
    state: &AppState,
    user_id: &str,
    tenant_id: &str,
    client_id: &str,
) -> impl IntoResponse {
    let ttl: u64 = std::env::var("JWT_TTL_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(900);

    let project = client_id
        .parse()
        .unwrap_or(aspectus_core::project::Project::Pandaria);

    // Expand scope from user roles
    let scopes = crate::scope_expander::ScopeExpander::expand(
        &state.pool,
        user_id,
        Some(&state.scope_cache),
    )
    .await;

    // ADR-016: Look up tenant info to embed in JWT + response.
    // These are best-effort — failures are logged but don't fail token issuance.
    let tenant_row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT name, logo_url FROM tenants WHERE id = $1")
            .bind(tenant_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);

    let tenant_name = tenant_row.as_ref().map(|(n, _)| n.clone());
    let tenant_logo_url = tenant_row.and_then(|(_, logo)| logo);

    let user_info: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT email, display_name FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);

    let (user_email, user_display_name) =
        user_info.map(|(e, d)| (Some(e), d)).unwrap_or((None, None));

    let access = match state
        .jwt_signer
        .sign_with_tenant_name(aspectus_auth::jwt::JwtSignRequest {
            sub: user_id.to_string(),
            tenant_id: tenant_id.to_string(),
            tenant_name: tenant_name.clone(),
            project,
            scopes: scopes.clone(),
            identity_type: IdentityType::User,
            ttl_seconds: ttl,
        }) {
        Ok(t) => t,
        Err(e) => {
            return ProblemDetails::from(e).into_response();
        }
    };

    // Issue refresh token
    let mut raw = [0u8; 32];
    if let Err(e) = getrandom::getrandom(&mut raw) {
        tracing::error!(error = %e, "Failed to generate refresh token");
        return ProblemDetails::internal_error("Token service temporarily unavailable")
            .into_response();
    }
    let refresh = format!("rt_{}", hex::encode(raw));
    let refresh_hash = hex::encode(Sha256::digest(refresh.as_bytes()));

    if let Err(e) = state
        .refresh_token_store
        .create(
            &refresh_hash,
            user_id,
            client_id,
            Utc::now() + chrono::Duration::days(30),
        )
        .await
    {
        tracing::error!(error = %e, "Failed to store refresh token");
        return ProblemDetails::internal_error("Token service temporarily unavailable")
            .into_response();
    }

    // ADR-016: Derive available_projects from the expanded scope string.
    // Reuses the helper so the parsing logic is unit-tested.
    let available_projects: Vec<String> = crate::scope_expander::projects_from_scopes(&scopes);

    // ADR-016: Enhanced response — user + tenant context so clients don't need
    // extra API calls to display "Acme Corp's alice".
    let mut response = json!({
        "access_token": access,
        "token_format": "jwt",
        "expires_in": ttl,
        "token_type": "Bearer",
        "refresh_token": refresh,
        "user": {
            "id": user_id,
            "email": user_email,
            "display_name": user_display_name,
        },
        "tenant": {
            "id": tenant_id,
            "name": tenant_name,
            "logo_url": tenant_logo_url,
        },
        "available_projects": available_projects,
    });

    // Backward compatibility: if tenant_name lookup failed, drop the field
    // (matches JWT behavior — skip_serializing_if on Option::None).
    if let Some(obj) = response.get_mut("tenant").and_then(|t| t.as_object_mut()) {
        if obj.get("name").map(|v| v.is_null()).unwrap_or(false) {
            obj.remove("name");
        }
        if obj.get("logo_url").map(|v| v.is_null()).unwrap_or(false) {
            obj.remove("logo_url");
        }
    }

    (StatusCode::OK, Json(response)).into_response()
}

// ---- /clients (v0.7) ----

#[derive(Deserialize)]
pub struct CreateClientRequest {
    name: String,
    redirect_uris: Vec<String>,
}

pub async fn create_client(
    State(state): State<AppState>,
    Json(req): Json<CreateClientRequest>,
) -> impl IntoResponse {
    let id = format!("client_{}", generate_id());

    // Generate a client_secret and store only its SHA-256 hash.
    // The plain-text secret is returned exactly once.
    let mut raw = [0u8; 32];
    if let Err(e) = getrandom::getrandom(&mut raw) {
        tracing::error!(error = %e, "Failed to generate client_secret");
        return ProblemDetails::internal_error("Failed to create client").into_response();
    }
    let secret = hex::encode(raw);
    let secret_hash = hex::encode(Sha256::digest(secret.as_bytes()));

    match state
        .oauth_client_store
        .create(&id, &req.name, &req.redirect_uris, &secret_hash)
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(json!({
                "client_id": id,
                "name": req.name,
                "client_secret": secret,
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create OAuth2 client");
            ProblemDetails::internal_error("Failed to create client").into_response()
        }
    }
}

pub async fn list_clients(State(state): State<AppState>) -> impl IntoResponse {
    match state.oauth_client_store.list().await {
        Ok(rows) => {
            let clients: Vec<serde_json::Value> = rows.into_iter().map(|(id, name, uris)| {
                json!({"client_id": id, "name": name, "redirect_uris": uris})
            }).collect();
            (StatusCode::OK, Json(clients)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list OAuth2 clients");
            (StatusCode::OK, Json(Vec::<serde_json::Value>::new())).into_response()
        }
    }
}
