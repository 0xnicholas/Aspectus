//! Tests for OpenAPI documentation endpoints.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn openapi_spec_returns_yaml() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/openapi.yaml")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 128 * 1024)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.starts_with("openapi:"), "spec should be YAML");
    assert!(
        text.contains("/introspect"),
        "spec should include /introspect"
    );
}

#[tokio::test]
async fn swagger_ui_returns_html() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/docs")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("swagger-ui"),
        "docs page should load swagger-ui"
    );
}
