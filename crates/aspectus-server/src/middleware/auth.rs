use std::sync::Arc;

use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use aspectus_auth::ServiceTokenVerifier;

use crate::error::ProblemDetails;

/// Middleware that authenticates requests via Service Token (ADR-011).
///
/// Extracts `Authorization: Bearer {token}` header and verifies it through
/// `ServiceTokenVerifier`. On success, injects the `Project` into request extensions.
pub async fn service_token_auth(
    mut request: Request,
    next: Next,
) -> Response {
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let Some(token) = auth_header else {
        return ProblemDetails::with_code_instance(
            aspectus_core::ErrorCode::InvalidServiceToken,
            "Missing Authorization header",
            request.uri().path(),
        )
        .into_response();
    };

    let verifier = request
        .extensions()
        .get::<Arc<ServiceTokenVerifier>>()
        .cloned();

    let Some(verifier) = verifier else {
        return ProblemDetails::internal_error("ServiceTokenVerifier not configured")
            .into_response();
    };

    match verifier.verify(token).await {
        Some(project) => {
            request.extensions_mut().insert(project);
            next.run(request).await
        }
        None => {
            ProblemDetails::with_code_instance(
                aspectus_core::ErrorCode::InvalidServiceToken,
                "Invalid Service Token",
                request.uri().path(),
            )
                .into_response()
        }
    }
}
