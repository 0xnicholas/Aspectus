//! HTTP-level tests for POST /introspect endpoint.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn health_returns_200() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_returns_200() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn jwks_returns_200() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/.well-known/jwks.json")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["keys"].is_array());
}

#[tokio::test]
async fn introspect_missing_service_token_returns_401() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/introspect")
        .method("POST")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from("token=pk_live_nonexistent"))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn introspect_wrong_service_token_returns_401() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/introspect")
        .method("POST")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", "Bearer wrong-token")
        .body(Body::from("token=pk_live_nonexistent"))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn introspect_unknown_key_returns_inactive() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/introspect")
        .method("POST")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", &common::service_token_header())
        .body(Body::from("token=pk_live_00000000000000000000000000000000"))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["active"], false);
}

#[tokio::test]
async fn introspect_malformed_token_returns_inactive() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/introspect")
        .method("POST")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", &common::service_token_header())
        .body(Body::from("token=not-a-valid-token-at-all"))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["active"], false);
}

#[tokio::test]
async fn introspect_jwt_token_malformed_returns_inactive() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/introspect")
        .method("POST")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", &common::service_token_header())
        .body(Body::from("token=eyJhbGciOiJSUzI1NiJ9.invalid"))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["active"], false);
}
