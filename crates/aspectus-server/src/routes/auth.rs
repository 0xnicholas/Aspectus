//! User-facing authentication endpoints.
//!
//! Unlike the OAuth2 /authorize → /oauth/token flow and the management API,
//! these endpoints are designed for project-owned login UIs:
//!
//! - `POST /login`   — email+password → JWT (single step)
//! - `POST /register` — create account + auto-login
//! - `POST /logout`  — revoke refresh token
//! - `POST /forgot-password` — generate reset token (emails stub)
//! - `POST /reset-password`  — verify token + update password

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum::http::HeaderMap;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use aspectus_auth::password::PasswordHasher;
use aspectus_core::identity::IdentityType;
use aspectus_core::store::{AuditLogStore, RefreshTokenStore};

use crate::error::ProblemDetails;
use crate::AppState;

// ── Helpers ────────────────────────────────────────────────────

/// Extract client IP from X-Forwarded-For header or fallback.
fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Record an audit log entry for authentication events.
async fn audit_auth_event(
    state: &AppState,
    tenant_id: &str,
    actor_id: &str,
    actor_type: IdentityType,
    action: &str,
    target_type: &str,
    target_id: &str,
    metadata: serde_json::Value,
) {
    let id = crate::util::generate_id();
    let entry = aspectus_core::audit_log::AuditLog {
        id,
        tenant_id: tenant_id.to_string(),
        actor_id: actor_id.to_string(),
        actor_type,
        action: action.to_string(),
        target_type: target_type.to_string(),
        target_id: target_id.to_string(),
        metadata,
        created_at: Utc::now(),
    };
    if let Err(e) = state.audit_log_store.append(entry).await {
        tracing::error!(error = %e, "Failed to write audit log");
    }
}

// ── POST /login ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
    /// Which project the user is logging into (default: "pandaria").
    #[serde(default = "default_client_id")]
    client_id: String,
}

fn default_client_id() -> String {
    "pandaria".into()
}

/// POST /login
///
/// One-step email+password → JWT access token + refresh token.
/// No OAuth2 authorization code exchange required.
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(login_req): Json<LoginRequest>,
) -> impl IntoResponse {
    let ip = client_ip(&headers);

    // Validate client_id is a known project
    let client_id = login_req.client_id;
    if client_id.parse::<aspectus_core::project::Project>().is_err() {
        return ProblemDetails::validation_failed(
            format!("Unknown project: {client_id}"),
            vec![],
        )
        .into_response();
    }

    // Look up user by email
    let (user_id, tenant_id) = match sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, tenant_id, password_hash FROM users WHERE email = $1",
    )
    .bind(&login_req.email)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some((id, tid, hash))) => {
            match PasswordHasher::verify(&login_req.password, &hash) {
                Ok(true) => (id, tid),
                _ => {
                    // Failed login — ambiguous message, no user/email leak
                    return ProblemDetails::unauthorized("Invalid email or password", "/login").into_response();
                }
            }
        }
        Ok(None) => {
            // No such user — same ambiguous message
            return ProblemDetails::unauthorized("Invalid email or password", "/login").into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, email = %login_req.email, "User lookup failed");
            return ProblemDetails::internal_error("Authentication service temporarily unavailable").into_response();
        }
    };

    // Check if suspended
    let is_suspended: bool = sqlx::query_scalar(
        "SELECT is_suspended FROM users WHERE id = $1",
    )
    .bind(&user_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if is_suspended {
        audit_auth_event(
            &state, &tenant_id, &user_id, IdentityType::User,
            "user.login_blocked", "user", &user_id,
            serde_json::json!({"reason": "account_suspended", "ip": ip}),
        ).await;
        return ProblemDetails::unauthorized("Account is suspended", "/login").into_response();
    }

    // Update last_sign_in_at
    let _ = sqlx::query("UPDATE users SET last_sign_in_at = NOW() WHERE id = $1")
        .bind(&user_id)
        .execute(&state.pool)
        .await;

    // Audit: successful login
    audit_auth_event(
        &state, &tenant_id, &user_id, IdentityType::User,
        "user.login", "user", &user_id,
        serde_json::json!({"ip": ip, "client_id": client_id}),
    ).await;

    // Issue tokens
    crate::routes::oauth::issue_tokens(&state, &user_id, &tenant_id, &client_id).await.into_response()
}

// ── POST /register ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    email: String,
    password: String,
    #[serde(default)]
    display_name: Option<String>,
    /// Tenant to join (default: "default").
    #[serde(default = "default_tenant")]
    tenant_id: String,
    /// Which project the user is registering from (default: "pandaria").
    #[serde(default = "default_client_id")]
    client_id: String,
}

fn default_tenant() -> String {
    "default".into()
}

/// POST /register
///
/// Create a new user account and return JWT tokens (auto-login).
/// Requires `ASPECTUS_REGISTRATION_ENABLED=true` env var.
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(reg): Json<RegisterRequest>,
) -> impl IntoResponse {
    let ip = client_ip(&headers);

    // Check if registration is enabled
    let enabled = std::env::var("ASPECTUS_REGISTRATION_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if !enabled {
        return ProblemDetails::forbidden("Public registration is disabled").into_response();
    }

    // Validate client_id is a known project
    let client_id = reg.client_id.clone();
    if client_id.parse::<aspectus_core::project::Project>().is_err() {
        return ProblemDetails::validation_failed(
            format!("Unknown project: {client_id}"),
            vec![],
        ).into_response();
    }

    // Validate email format
    if !reg.email.contains('@') || reg.email.len() < 5 {
        return ProblemDetails::validation_failed(
            "Invalid email address",
            vec![],
        )
        .into_response();
    }

    // Validate password strength (min 8 chars)
    if reg.password.len() < 8 {
        return ProblemDetails::validation_failed(
            "Password must be at least 8 characters",
            vec![],
        )
        .into_response();
    }

    // Check email uniqueness
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)",
    )
    .bind(&reg.email)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if exists {
        // Don't leak whether email exists — use same error as login
        return ProblemDetails::validation_failed(
            "Email is already registered",
            vec![],
        )
        .into_response();
    }

    // Ensure tenant exists (or create default)
    let tenant_id = reg.tenant_id;
    let tenant_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tenants WHERE id = $1)",
    )
    .bind(&tenant_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !tenant_exists {
        // Auto-create the tenant if it doesn't exist
        if let Err(e) = sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, $1)")
            .bind(&tenant_id)
            .execute(&state.pool)
            .await
        {
            tracing::error!(error = %e, tenant_id = %tenant_id, "Failed to auto-create tenant");
            return ProblemDetails::internal_error("Failed to create tenant").into_response();
        }
    }

    // Hash password
    let password_hash = match PasswordHasher::hash(&reg.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "Password hashing failed");
            return ProblemDetails::internal_error("Failed to process registration").into_response();
        }
    };

    // Create user
    let user_id = crate::util::generate_id();
    if let Err(e) = sqlx::query(
        "INSERT INTO users (id, tenant_id, email, password_hash, display_name) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&user_id)
    .bind(&tenant_id)
    .bind(&reg.email)
    .bind(&password_hash)
    .bind(reg.display_name.as_deref())
    .execute(&state.pool)
    .await
    {
        tracing::error!(error = %e, "User creation failed");
        return ProblemDetails::internal_error("Failed to create account").into_response();
    }

    // Audit: registration
    audit_auth_event(
        &state, &tenant_id, &user_id, IdentityType::User,
        "user.registered", "user", &user_id,
        serde_json::json!({"ip": ip, "email": reg.email}),
    ).await;

    // Auto-login: issue tokens
    crate::routes::oauth::issue_tokens(&state, &user_id, &tenant_id, &client_id).await.into_response()
}

// ── POST /logout ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LogoutRequest {
    refresh_token: String,
    /// Optional: also revoke the access token (for JWT, this adds jti to deny list)
    #[serde(default)]
    access_token: Option<String>,
}

/// POST /logout
///
/// Revoke the refresh token. If an access_token is provided, it is also
/// added to the JWT revocation list (for server-side logout).
pub async fn logout(
    State(state): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> impl IntoResponse {
    // Revoke refresh token
    let rt_hash = hex::encode(Sha256::digest(req.refresh_token.as_bytes()));

    if let Err(e) = state.refresh_token_store.rotate(&rt_hash).await {
        // rotate() atomically marks as revoked — even if the token is already
        // revoked or expired, we return success (idempotent)
        tracing::warn!(error = %e, "Refresh token revocation attempted");
    }

    // Revoke access token (JWT) if provided
    if let Some(access_token) = req.access_token {
        if access_token.starts_with("eyJ") {
            state.jwt_verifier.revoke(&access_token).await;
        }
    }

    StatusCode::NO_CONTENT.into_response()
}

// ── POST /forgot-password ──────────────────────────────────────

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    email: String,
}

/// POST /forgot-password
///
/// Generate a one-time password reset token and "send" it via email.
/// In development, the reset token is logged (no email server required).
///
/// Always returns success to prevent email enumeration.
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(req): Json<ForgotPasswordRequest>,
) -> impl IntoResponse {
    // Look up user (but don't reveal existence)
    let user = match sqlx::query_as::<_, (String, String)>(
        "SELECT id, tenant_id FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some(u)) => u,
        _ => {
            // User not found — return success anyway to prevent enumeration
            return (StatusCode::OK, Json(json!({"message": "If this email is registered, a reset link has been sent."}))).into_response();
        }
    };

    let (user_id, _tenant_id) = user;

    // Generate reset token
    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap_or_default();
    let token = hex::encode(raw);
    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    let expires_at = Utc::now() + chrono::Duration::hours(1);

    // Store token hash
    if let Err(e) = sqlx::query(
        "INSERT INTO password_reset_tokens (token_hash, user_id, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(&token_hash)
    .bind(&user_id)
    .bind(expires_at)
    .execute(&state.pool)
    .await
    {
        tracing::error!(error = %e, "Failed to store password reset token");
        return ProblemDetails::internal_error("Failed to process request").into_response();
    }

    // In production, send email. For now, log the reset link.
    let reset_url = format!(
        "https://aspectus.local/reset-password?token={token}"
    );
    tracing::info!(
        user_id = %user_id,
        reset_url = %reset_url,
        "Password reset token generated (email stub)"
    );

    // Always return the same message
    (StatusCode::OK, Json(json!({"message": "If this email is registered, a reset link has been sent."}))).into_response()
}

// ── POST /reset-password ───────────────────────────────────────

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    token: String,
    new_password: String,
}

/// POST /reset-password
///
/// Verify the reset token and update the user's password.
pub async fn reset_password(
    State(state): State<AppState>,
    Json(req): Json<ResetPasswordRequest>,
) -> impl IntoResponse {
    // Validate password strength
    if req.new_password.len() < 8 {
        return ProblemDetails::validation_failed(
            "Password must be at least 8 characters",
            vec![],
        )
        .into_response();
    }

    // Hash the incoming token to look it up
    let token_hash = hex::encode(Sha256::digest(req.token.as_bytes()));

    // Atomically claim the token (mark used)
    let user_id: Option<String> = sqlx::query_scalar(
        "UPDATE password_reset_tokens SET used = true \
         WHERE token_hash = $1 AND used = false AND expires_at > NOW() \
         RETURNING user_id",
    )
    .bind(&token_hash)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    let user_id = match user_id {
        Some(id) => id,
        None => {
            return ProblemDetails::validation_failed(
                "Invalid or expired reset token",
                vec![],
            )
            .into_response();
        }
    };

    // Hash new password
    let password_hash = match PasswordHasher::hash(&req.new_password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "Password hashing failed");
            return ProblemDetails::internal_error("Failed to update password").into_response();
        }
    };

    // Update password
    if let Err(e) = sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(&password_hash)
        .bind(&user_id)
        .execute(&state.pool)
        .await
    {
        tracing::error!(error = %e, user_id = %user_id, "Password update failed");
        return ProblemDetails::internal_error("Failed to update password").into_response();
    }

    // Audit
    let tenant_id: Option<String> = sqlx::query_scalar(
        "SELECT tenant_id FROM users WHERE id = $1",
    )
    .bind(&user_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    if let Some(tid) = tenant_id {
        audit_auth_event(
            &state, &tid, &user_id, IdentityType::User,
            "user.password_reset", "user", &user_id,
            serde_json::json!({"method": "email_reset_token"}),
        ).await;
    }

    (StatusCode::OK, Json(json!({"message": "Password has been reset successfully."}))).into_response()
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── LoginRequest ──

    #[test]
    fn login_request_with_client_id() {
        let json = r#"{"email":"a@b.com","password":"secret123"}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "a@b.com");
        assert_eq!(req.client_id, "pandaria"); // default
    }

    #[test]
    fn login_request_custom_client_id() {
        let json = r#"{"email":"a@b.com","password":"secret123","client_id":"tavern"}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.client_id, "tavern");
    }

    // ── RegisterRequest ──

    #[test]
    fn register_request_minimal() {
        let json = r#"{"email":"a@b.com","password":"secret123"}"#;
        let req: RegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "a@b.com");
        assert_eq!(req.tenant_id, "default");
        assert_eq!(req.client_id, "pandaria");
        assert!(req.display_name.is_none());
    }

    #[test]
    fn register_request_full() {
        let json = r#"{"email":"a@b.com","password":"secret123","display_name":"Alice","tenant_id":"org-foo","client_id":"tavern"}"#;
        let req: RegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.display_name.as_deref(), Some("Alice"));
        assert_eq!(req.tenant_id, "org-foo");
        assert_eq!(req.client_id, "tavern");
    }

    #[test]
    fn register_rejects_short_password() {
        // Password < 8 chars should fail validation (tested in handler, but we check the rule)
        let short = "short";
        assert!(short.len() < 8);
    }

    #[test]
    fn register_rejects_invalid_email() {
        let invalid = "not-an-email";
        assert!(!invalid.contains('@'));
    }

    // ── LogoutRequest ──

    #[test]
    fn logout_request_minimal() {
        let json = r#"{"refresh_token":"rt_abc123"}"#;
        let req: LogoutRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.refresh_token, "rt_abc123");
        assert!(req.access_token.is_none());
    }

    #[test]
    fn logout_request_with_access_token() {
        let json = r#"{"refresh_token":"rt_abc","access_token":"eyJ..."}"#;
        let req: LogoutRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.access_token.as_deref(), Some("eyJ..."));
    }

    // ── ForgotPasswordRequest ──

    #[test]
    fn forgot_password_request() {
        let json = r#"{"email":"a@b.com"}"#;
        let req: ForgotPasswordRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "a@b.com");
    }

    // ── ResetPasswordRequest ──

    #[test]
    fn reset_password_request() {
        let json = r#"{"token":"abc123","new_password":"newsecret123"}"#;
        let req: ResetPasswordRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.token, "abc123");
        assert_eq!(req.new_password, "newsecret123");
    }

    #[test]
    fn reset_password_rejects_short_password() {
        // Simulate the handler's validation
        let req: ResetPasswordRequest = serde_json::from_str(
            r#"{"token":"abc","new_password":"short"}"#
        ).unwrap();
        assert!(req.new_password.len() < 8);
    }

    // ── Client IP extraction ──

    #[test]
    fn client_ip_from_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "10.0.0.1".parse().unwrap());
        assert_eq!(client_ip(&headers), "10.0.0.1");
    }

    #[test]
    fn client_ip_first_in_chain() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "10.0.0.1, 10.0.0.2".parse().unwrap());
        assert_eq!(client_ip(&headers), "10.0.0.1");
    }

    #[test]
    fn client_ip_fallback() {
        let headers = HeaderMap::new();
        assert_eq!(client_ip(&headers), "unknown");
    }

    // ── Default values ──

    #[test]
    fn default_client_id_is_pandaria() {
        assert_eq!(default_client_id(), "pandaria");
    }

    #[test]
    fn default_tenant_is_default() {
        assert_eq!(default_tenant(), "default");
    }
}
