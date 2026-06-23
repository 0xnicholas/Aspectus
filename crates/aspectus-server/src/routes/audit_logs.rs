//! Audit log query endpoint.
//!
//! Provides read-only access to the append-only audit log. All management
//! actions that write to `audit_logs` should be queryable here.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use aspectus_core::{
    store::{AuditLogFilter, AuditLogStore},
};

use crate::error::ProblemDetails;
use crate::AppState;

const MAX_AUDIT_LIMIT: i64 = 1000;

pub async fn list(
    State(state): State<AppState>,
    Query(filter): Query<AuditLogFilter>,
) -> impl IntoResponse {
    if filter.limit <= 0 || filter.limit > MAX_AUDIT_LIMIT {
        return ProblemDetails::validation_failed(
            format!("limit must be between 1 and {MAX_AUDIT_LIMIT}"),
            vec![],
        )
        .into_response();
    }
    if filter.offset < 0 {
        return ProblemDetails::validation_failed(
            "offset must be non-negative",
            vec![],
        )
        .into_response();
    }

    match state.audit_log_store.list(filter).await {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to query audit logs");
            ProblemDetails::internal_error("Failed to query audit logs").into_response()
        }
    }
}
