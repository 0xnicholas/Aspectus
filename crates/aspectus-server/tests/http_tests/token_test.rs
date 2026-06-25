//! HTTP-level tests for `POST /token` and `POST /token/revoke`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::common;

async fn create_tenant(app: &axum::Router, name: &str) -> String {
    let req = Request::builder()
        .uri("/tenants")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"name": name}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let tenant: serde_json::Value = serde_json::from_slice(&body).unwrap();
    tenant["id"].as_str().unwrap().to_string()
}

async fn create_service_account(app: &axum::Router, tenant_id: &str, label: &str) -> String {
    let req = Request::builder()
        .uri("/service-accounts")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({"tenant_id": tenant_id, "label": label}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let sa: serde_json::Value = serde_json::from_slice(&body).unwrap();
    sa["id"].as_str().unwrap().to_string()
}

async fn create_api_key(
    app: &axum::Router,
    tenant_id: &str,
    service_account_id: &str,
    project: &str,
    scopes: Vec<&str>,
) -> String {
    let req = Request::builder()
        .uri("/api-keys")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": tenant_id,
                "owner_type": "service_account",
                "owner_id": service_account_id,
                "project": project,
                "scopes": scopes,
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "api-key create failed");
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let key: serde_json::Value = serde_json::from_slice(&body).unwrap();
    key["key"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn token_issue_jwt_and_introspect() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "token-issue-jwt").await;
    let sa_id = create_service_account(&app, &tenant_id, "token-sa").await;
    let key = create_api_key(
        &app,
        &tenant_id,
        &sa_id,
        "pandaria",
        vec!["pandaria:session:read"],
    )
    .await;

    let req = Request::builder()
        .uri("/token")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "grant_type": "client_credentials",
                "client_id": "pandaria",
                "client_secret": key,
                "token_format": "jwt"
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let resp_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let access_token = resp_json["access_token"].as_str().unwrap();
    assert!(access_token.starts_with("eyJ"));
    assert_eq!(resp_json["token_format"], "jwt");

    // Introspect the issued JWT using the consumer service token.
    let req = Request::builder()
        .uri("/introspect")
        .method("POST")
        .header("Authorization", &common::service_token_header())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from(format!("token={access_token}")))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let introspect: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(introspect["active"].as_bool().unwrap());
    assert_eq!(introspect["tenant_id"], tenant_id);
    assert!(
        introspect["scope"]
            .as_str()
            .unwrap()
            .contains("pandaria:session:read")
    );
}

#[tokio::test]
async fn token_issue_opaque() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "token-issue-opaque").await;
    let sa_id = create_service_account(&app, &tenant_id, "token-sa-opaque").await;
    let key = create_api_key(
        &app,
        &tenant_id,
        &sa_id,
        "pandaria",
        vec!["pandaria:session:read"],
    )
    .await;

    let req = Request::builder()
        .uri("/token")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "grant_type": "client_credentials",
                "client_id": "pandaria",
                "client_secret": key,
                "token_format": "opaque"
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let resp_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let access_token = resp_json["access_token"].as_str().unwrap();
    assert!(access_token.starts_with("ot_"));
    assert_eq!(resp_json["token_format"], "opaque");
}

#[tokio::test]
async fn token_rejects_client_id_project_mismatch() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "token-mismatch").await;
    let sa_id = create_service_account(&app, &tenant_id, "token-sa-mismatch").await;
    let key = create_api_key(
        &app,
        &tenant_id,
        &sa_id,
        "pandaria",
        vec!["pandaria:session:read"],
    )
    .await;

    let req = Request::builder()
        .uri("/token")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "grant_type": "client_credentials",
                "client_id": "constell",
                "client_secret": key,
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_rejects_invalid_client_secret() {
    let (app, _) = common::build_app().await.unwrap();

    let req = Request::builder()
        .uri("/token")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "grant_type": "client_credentials",
                "client_id": "pandaria",
                "client_secret": "pk_live_0000000000000000000000000000000000000000000000000000000000000000",
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_revoke_requires_service_token_auth() {
    let (app, _) = common::build_app().await.unwrap();

    let req = Request::builder()
        .uri("/token/revoke")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({"token": "pk_live_xxx"}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_key_creation_rejects_scope_project_mismatch() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "scope-mismatch").await;
    let sa_id = create_service_account(&app, &tenant_id, "scope-sa").await;

    let req = Request::builder()
        .uri("/api-keys")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": tenant_id,
                "owner_type": "service_account",
                "owner_id": sa_id,
                "project": "pandaria",
                "scopes": ["constell:agent:read"],
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn tenant_quota_update_rejects_invalid_schema() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "quota-schema").await;

    let req = Request::builder()
        .uri(format!("/tenants/{tenant_id}/quotas"))
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!("not-an-object").to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Unknown project key should also be rejected.
    let req = Request::builder()
        .uri(format!("/tenants/{tenant_id}/quotas"))
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"tavern": {"x": 1}}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn token_revoke_makes_api_key_inactive() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "token-revoke").await;
    let sa_id = create_service_account(&app, &tenant_id, "token-sa-revoke").await;
    let key = create_api_key(
        &app,
        &tenant_id,
        &sa_id,
        "pandaria",
        vec!["pandaria:session:read"],
    )
    .await;

    // Revoke via /token/revoke with consumer service token.
    let req = Request::builder()
        .uri("/token/revoke")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(json!({"token": key}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Introspect should now be inactive.
    let req = Request::builder()
        .uri("/introspect")
        .method("POST")
        .header("Authorization", &common::service_token_header())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from(format!("token={key}")))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let introspect: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(!introspect["active"].as_bool().unwrap());
}
