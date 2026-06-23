//! Integration tests for the audit log query endpoint.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::SecondsFormat;
use serde_json::{json, Value};
use tower::ServiceExt;

use super::common;

fn admin_auth() -> String {
    common::admin_service_token_header()
}

async fn create_service_token(app: &axum::Router, project: &str) -> Value {
    let req = Request::builder()
        .uri("/service-tokens")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", admin_auth())
        .body(Body::from(json!({"project": project}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn query_audit_logs(app: &axum::Router, query: &str) -> (StatusCode, Value) {
    let uri = if query.is_empty() {
        "/audit-logs".to_string()
    } else {
        format!("/audit-logs?{query}")
    };
    let req = Request::builder()
        .uri(&uri)
        .method("GET")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap_or(json!(null));
    (status, value)
}

async fn cleanup_service_token(app: &axum::Router, project: &str) {
    let req = Request::builder()
        .uri(format!("/service-tokens/{project}"))
        .method("DELETE")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let _ = app.clone().oneshot(req).await;
}

#[tokio::test]
async fn list_includes_service_token_events() {
    let app = common::build_app_with().await.unwrap().router;
    let project = "constell";
    cleanup_service_token(&app, project).await;

    create_service_token(&app, project).await;

    let (status, logs) = query_audit_logs(&app, "action=service_token.created").await;
    assert_eq!(status, StatusCode::OK);
    let logs = logs.as_array().expect("audit logs array");
    assert!(
        logs.iter().any(|entry| {
            entry["action"] == "service_token.created"
                && entry["tenant_id"] == "system"
                && entry["target_id"] == project
        }),
        "expected service_token.created audit entry"
    );

    cleanup_service_token(&app, project).await;
}

#[tokio::test]
async fn filter_by_tenant_id() {
    let app = common::build_app_with().await.unwrap().router;
    let project = "tokencamp";
    cleanup_service_token(&app, project).await;

    create_service_token(&app, project).await;

    // System tenant filter should include the service token event.
    let (status, logs) = query_audit_logs(&app, "tenant_id=system").await;
    assert_eq!(status, StatusCode::OK);
    let logs = logs.as_array().unwrap();
    assert!(logs.iter().any(|e| e["action"] == "service_token.created"));

    // A tenant that only has its own events should not include system events.
    // Create a tenant and check its audit log is separate.
    let req = Request::builder()
        .uri("/tenants")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", admin_auth())
        .body(Body::from(json!({"name": "audit-tenant-test"}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let tenant: Value = serde_json::from_slice(&body).unwrap();
    let tenant_id = tenant["id"].as_str().unwrap();

    let (status, logs) = query_audit_logs(&app, &format!("tenant_id={tenant_id}&action=tenant.created")).await;
    assert_eq!(status, StatusCode::OK);
    let logs = logs.as_array().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["target_id"], tenant_id);

    cleanup_service_token(&app, project).await;
}

#[tokio::test]
async fn pagination_and_limit_validation() {
    let app = common::build_app_with().await.unwrap().router;

    // limit=0 and limit>MAX should be rejected.
    let (status, _) = query_audit_logs(&app, "limit=0").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = query_audit_logs(&app, "limit=1001").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = query_audit_logs(&app, "offset=-1").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Valid pagination should work.
    let (status, logs) = query_audit_logs(&app, "limit=1&offset=0").await;
    assert_eq!(status, StatusCode::OK);
    assert!(logs.as_array().unwrap().len() <= 1);
}

#[tokio::test]
async fn time_range_filter() {
    let app = common::build_app_with().await.unwrap().router;
    let project = "emerald";
    cleanup_service_token(&app, project).await;

    let before = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    create_service_token(&app, project).await;
    let after = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    let (status, logs) = query_audit_logs(
        &app,
        &format!("action=service_token.created&from={before}&to={after}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(logs
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["target_id"] == project));

    // Querying a time range in the past should return nothing for this event.
    let old_to = (chrono::Utc::now() - chrono::Duration::hours(1))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    let (status, logs) = query_audit_logs(
        &app,
        &format!("action=service_token.created&to={old_to}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!logs
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["target_id"] == project));

    cleanup_service_token(&app, project).await;
}

#[tokio::test]
async fn unauthorized_or_forbidden_requests_are_rejected() {
    let app = common::build_app_with().await.unwrap().router;

    let req = Request::builder()
        .uri("/audit-logs")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let req = Request::builder()
        .uri("/audit-logs")
        .method("GET")
        .header("Authorization", common::service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
