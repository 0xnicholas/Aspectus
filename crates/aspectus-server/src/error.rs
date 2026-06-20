use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// RFC 7807 Problem Details error response (ADR-014).
///
/// All management API errors use this format.
/// `/introspect` subject-token errors use RFC 7662 `{active: false}` instead.
///
/// The `code` field carries a stable, machine-readable error code (see
/// `aspectus_core::ErrorCode`) that clients can branch on without parsing
/// human-readable messages.
#[derive(Debug, Serialize)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub type_uri: String,

    pub title: String,

    pub status: u16,

    /// Stable machine-readable error code (e.g. "user_not_found").
    /// Maps to `aspectus_core::ErrorCode::as_str()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    pub detail: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ValidationError>>,
}

#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl ProblemDetails {
    /// Create with a specific error code. Sets `type_uri` and `status` from the code.
    pub fn with_code(code: aspectus_core::ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            type_uri: code.type_uri(),
            title: Self::title_for_code(code),
            status: code.http_status(),
            code: Some(code.as_str().into()),
            detail: detail.into(),
            instance: None,
            errors: None,
        }
    }

    /// Create with code + instance path.
    pub fn with_code_instance(
        code: aspectus_core::ErrorCode,
        detail: impl Into<String>,
        instance: impl Into<String>,
    ) -> Self {
        let mut pd = Self::with_code(code, detail);
        pd.instance = Some(instance.into());
        pd
    }

    /// Create with code + validation errors list.
    pub fn with_code_errors(
        code: aspectus_core::ErrorCode,
        detail: impl Into<String>,
        errors: Vec<ValidationError>,
    ) -> Self {
        let mut pd = Self::with_code(code, detail);
        pd.errors = Some(errors);
        pd
    }

    fn title_for_code(code: aspectus_core::ErrorCode) -> String {
        match code.http_status() {
            401 => "Unauthorized".into(),
            403 => "Forbidden".into(),
            404 => "Not Found".into(),
            422 => "Validation Failed".into(),
            429 => "Too Many Requests".into(),
            _ => "Internal Server Error".into(),
        }
    }

    // ---- Backward-compatible constructors (no code) ----

    pub fn unauthorized(detail: impl Into<String>, instance: impl Into<String>) -> Self {
        Self::with_code_instance(
            aspectus_core::ErrorCode::InvalidCredentials,
            detail,
            instance,
        )
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::with_code(aspectus_core::ErrorCode::RegistrationDisabled, detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::with_code(aspectus_core::ErrorCode::NotFound, detail)
    }

    pub fn validation_failed(detail: impl Into<String>, errors: Vec<ValidationError>) -> Self {
        Self::with_code_errors(aspectus_core::ErrorCode::ValidationFailed, detail, errors)
    }

    pub fn internal_error(detail: impl Into<String>) -> Self {
        Self::with_code(aspectus_core::ErrorCode::InternalError, detail)
    }
}

impl IntoResponse for ProblemDetails {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = Json(self).into_response();
        *response.status_mut() = status;
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}

impl From<aspectus_core::error::CoreError> for ProblemDetails {
    fn from(err: aspectus_core::error::CoreError) -> Self {
        match &err {
            aspectus_core::error::CoreError::NotFound { entity, id } => {
                Self::not_found(format!("{entity} not found: {id}"))
            }
            aspectus_core::error::CoreError::Validation(msg) => {
                Self::validation_failed(msg.clone(), vec![])
            }
            _ => Self::internal_error(err.to_string()),
        }
    }
}
