//! Integration tests for Service Token management endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use super::common;

fn admin_auth() -> String {
    common::admin_service_token_header()
}

async fn create_token(app: &axum::Router, project: &str) -> Value {
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

async fn cleanup_project(app: &axum::Router, project: &str) {
    let req = Request::builder()
        .uri(format!("/service-tokens/{project}"))
        .method("DELETE")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let _ = app.clone().oneshot(req).await;
}

async fn introspect_with_token(app: &axum::Router, service_token: &str) -> StatusCode {
    let req = Request::builder()
        .uri("/introspect")
        .method("POST")
        .header("Authorization", format!("Bearer {service_token}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from("token=not-a-real-token"))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    resp.status()
}

#[tokio::test]
async fn create_list_get_service_token_flow() {
    let app = common::build_app_with().await.unwrap().router;
    let project = "constell";
    cleanup_project(&app, project).await;

    // Create
    let created: Value = create_token(&app, project).await;
    assert_eq!(created["project"], project);
    assert!(created["token"].as_str().unwrap().starts_with("st_"));
    assert!(created["token_prefix"].as_str().unwrap().starts_with("st_"));

    // List
    let req = Request::builder()
        .uri("/service-tokens")
        .method("GET")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let list: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert!(list.iter().any(|t| t["project"] == project));

    // Get
    let req = Request::builder()
        .uri(format!("/service-tokens/{project}"))
        .method("GET")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let got: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(got["project"], project);
    assert!(got["token"].is_null());

    cleanup_project(&app, project).await;
}

#[tokio::test]
async fn duplicate_create_for_active_token_returns_conflict() {
    let app = common::build_app_with().await.unwrap().router;
    let project = "tokencamp";
    cleanup_project(&app, project).await;

    create_token(&app, project).await;

    let req = Request::builder()
        .uri("/service-tokens")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", admin_auth())
        .body(Body::from(json!({"project": project}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    cleanup_project(&app, project).await;
}

#[tokio::test]
async fn rotate_invalidates_old_token_immediately() {
    let app = common::build_app_with().await.unwrap().router;
    let project = "emerald";
    cleanup_project(&app, project).await;

    let created = create_token(&app, project).await;
    let old_token = created["token"].as_str().unwrap().to_string();

    // Old token works before rotation
    assert_eq!(introspect_with_token(&app, &old_token).await, StatusCode::OK);

    // Rotate
    let req = Request::builder()
        .uri(format!("/service-tokens/{project}/rotate"))
        .method("POST")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let rotated: Value = serde_json::from_slice(&body).unwrap();
    let new_token = rotated["token"].as_str().unwrap().to_string();
    assert_ne!(old_token, new_token);

    // Old token is rejected, new token works
    assert_eq!(introspect_with_token(&app, &old_token).await, StatusCode::UNAUTHORIZED);
    assert_eq!(introspect_with_token(&app, &new_token).await, StatusCode::OK);

    cleanup_project(&app, project).await;
}

#[tokio::test]
async fn revoke_blocks_token_and_shows_revoked_at() {
    let app = common::build_app_with().await.unwrap().router;
    let project = "heirloom";
    cleanup_project(&app, project).await;

    let created = create_token(&app, project).await;
    let token = created["token"].as_str().unwrap().to_string();
    assert_eq!(introspect_with_token(&app, &token).await, StatusCode::OK);

    // Revoke
    let req = Request::builder()
        .uri(format!("/service-tokens/{project}"))
        .method("DELETE")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Token is now rejected
    assert_eq!(introspect_with_token(&app, &token).await, StatusCode::UNAUTHORIZED);

    // Metadata shows revoked_at
    let req = Request::builder()
        .uri(format!("/service-tokens/{project}"))
        .method("GET")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let got: Value = serde_json::from_slice(&body).unwrap();
    assert!(got["revoked_at"].is_string());
}

#[tokio::test]
async fn missing_project_returns_not_found() {
    let app = common::build_app_with().await.unwrap().router;

    for method in ["GET", "DELETE"] {
        let req = Request::builder()
            .uri("/service-tokens/nonexistent")
            .method(method)
            .header("Authorization", admin_auth())
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{method} should return 404");
    }

    let req = Request::builder()
        .uri("/service-tokens/nonexistent/rotate")
        .method("POST")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn aspectus_token_cannot_be_managed() {
    let app = common::build_app_with().await.unwrap().router;

    // Create
    let req = Request::builder()
        .uri("/service-tokens")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", admin_auth())
        .body(Body::from(json!({"project": "aspectus"}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Rotate
    let req = Request::builder()
        .uri("/service-tokens/aspectus/rotate")
        .method("POST")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Revoke
    let req = Request::builder()
        .uri("/service-tokens/aspectus")
        .method("DELETE")
        .header("Authorization", admin_auth())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn management_requires_admin_service_token() {
    let app = common::build_app_with().await.unwrap().router;

    // No auth header
    let req = Request::builder()
        .uri("/service-tokens")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Consumer service token (pandaria) is forbidden for management
    let req = Request::builder()
        .uri("/service-tokens")
        .method("GET")
        .header("Authorization", common::service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
