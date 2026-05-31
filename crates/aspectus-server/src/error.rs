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
#[derive(Debug, Serialize)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub type_uri: String,

    pub title: String,

    pub status: u16,

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
    pub fn unauthorized(detail: impl Into<String>, instance: impl Into<String>) -> Self {
        Self {
            type_uri: "https://aspectus.dev/errors/unauthorized".into(),
            title: "Unauthorized".into(),
            status: 401,
            detail: detail.into(),
            instance: Some(instance.into()),
            errors: None,
        }
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self {
            type_uri: "https://aspectus.dev/errors/forbidden".into(),
            title: "Forbidden".into(),
            status: 403,
            detail: detail.into(),
            instance: None,
            errors: None,
        }
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self {
            type_uri: "https://aspectus.dev/errors/not-found".into(),
            title: "Not Found".into(),
            status: 404,
            detail: detail.into(),
            instance: None,
            errors: None,
        }
    }

    pub fn validation_failed(detail: impl Into<String>, errors: Vec<ValidationError>) -> Self {
        Self {
            type_uri: "https://aspectus.dev/errors/validation-failed".into(),
            title: "Validation Failed".into(),
            status: 422,
            detail: detail.into(),
            instance: None,
            errors: Some(errors),
        }
    }

    pub fn internal_error(detail: impl Into<String>) -> Self {
        Self {
            type_uri: "https://aspectus.dev/errors/internal-error".into(),
            title: "Internal Server Error".into(),
            status: 500,
            detail: detail.into(),
            instance: None,
            errors: None,
        }
    }
}

impl IntoResponse for ProblemDetails {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = Json(self).into_response();
        *response.status_mut() = status;
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            "application/problem+json".parse().unwrap(),
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
