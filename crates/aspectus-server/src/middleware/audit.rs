use std::sync::Arc;

use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use chrono::Utc;
use serde_json::json;

use aspectus_core::{
    audit_log::AuditLog, identity::IdentityType, project::Project, store::AuditLogStore,
};

use crate::util::generate_id;

/// Best-effort extraction of `tenant_id` from path, query, or JSON body.
///
/// Examples:
/// - `/tenants/{tenant_id}` / `/tenants/{tenant_id}/quotas`
/// - `?tenant_id={tenant_id}`
/// - JSON body field `tenant_id`
fn extract_tenant_id(path: &str, query: Option<&str>, body: Option<&[u8]>) -> Option<String> {
    // Path: /tenants/{tenant_id}/...
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() >= 2 && segs[0] == "tenants" && !segs[1].is_empty() {
        return Some(segs[1].to_string());
    }

    // Query string.
    if let Some(q) = query {
        for pair in q.split('&') {
            if let Some((k, v)) = pair.split_once('=')
                && k == "tenant_id"
                && !v.is_empty()
            {
                return Some(v.to_string());
            }
        }
    }

    // JSON body.
    if let Some(b) = body
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(b)
        && let Some(tenant_id) = value.get("tenant_id").and_then(|v| v.as_str())
        && !tenant_id.is_empty()
    {
        return Some(tenant_id.to_string());
    }

    None
}

/// Middleware that records every management API call to the audit log.
///
/// Inspired by Logto's `koa-audit-log`. Must run AFTER `service_token_auth`
/// in the middleware stack so the authenticated `Project` is available.
///
/// # Prerequisites
///
/// The caller must inject `Arc<dyn AuditLogStore>` into request extensions
/// before this middleware. See `inject_audit_store()` helper.
pub async fn audit_mgmt_api(request: Request, next: Next) -> Response {
    // Snapshot everything we need BEFORE the handler consumes the request.
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(|q| q.to_string());
    let actor_project = request.extensions().get::<Project>().copied();
    let audit_store = request
        .extensions()
        .get::<Arc<dyn AuditLogStore>>()
        .cloned();

    // Buffer a small copy of the body so we can extract tenant_id and still
    // pass the request downstream. Management API payloads are bounded by
    // DefaultBodyLimit (16 KiB) so this is safe.
    let (parts, body) = request.into_parts();
    let body_bytes = match axum::body::to_bytes(body, 1024 * 16).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "Audit middleware failed to buffer request body");
            return next.run(Request::from_parts(parts, Body::empty())).await;
        }
    };
    let tenant_id = extract_tenant_id(&path, query.as_deref(), Some(&body_bytes));
    let request = Request::from_parts(parts, Body::from(body_bytes));

    // Run the handler.
    let response: Response = next.run(request).await;

    // Silent return for health/metrics probes.
    let status = response.status().as_u16();
    if status < 400 && (path == "/health" || path == "/metrics") {
        return response;
    }

    // Fire-and-forget audit write.
    if let Some(store) = audit_store {
        let actor_id = actor_project
            .map(|p| p.to_string())
            .unwrap_or_else(|| "unknown".into());

        let action = if status >= 500 {
            format!("mgmt:{}:{}:error", method.to_lowercase(), path)
        } else if status >= 400 {
            format!("mgmt:{}:{}:denied", method.to_lowercase(), path)
        } else {
            format!("mgmt:{}:{}", method.to_lowercase(), path)
        };

        let tenant_id_for_log = tenant_id.unwrap_or_default();
        tokio::spawn(async move {
            let entry = AuditLog {
                id: generate_id(),
                tenant_id: tenant_id_for_log,
                actor_id,
                actor_type: IdentityType::ServiceAccount,
                action,
                target_type: "endpoint".into(),
                target_id: path.clone(),
                metadata: json!({ "method": method, "status": status }),
                created_at: Utc::now(),
            };
            if let Err(e) = store.append(entry).await {
                tracing::warn!(error = %e, "Audit middleware write failed");
            }
        });
    }

    response
}

/// Convenience constructor that injects the AuditLogStore into request
/// extensions and delegates to `audit_mgmt_api`.
///
/// Usage in `main.rs`:
/// ```ignore
/// .layer(middleware::from_fn(audit_layer(audit_store.clone())))
/// ```
pub fn audit_layer<S: AuditLogStore + 'static>(
    store: Arc<S>,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
+ Clone {
    let store: Arc<dyn AuditLogStore> = store;
    move |mut req: Request, next: Next| {
        let store = store.clone();
        Box::pin(async move {
            req.extensions_mut().insert(store);
            audit_mgmt_api(req, next).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_tenant_id_from_path() {
        assert_eq!(
            extract_tenant_id("/tenants/t1/quotas", None, None),
            Some("t1".into())
        );
        assert_eq!(
            extract_tenant_id("/tenants/t1", None, None),
            Some("t1".into())
        );
        assert_eq!(extract_tenant_id("/users/u1", None, None), None);
    }

    #[test]
    fn extract_tenant_id_from_query() {
        assert_eq!(
            extract_tenant_id("/users", Some("tenant_id=t1"), None),
            Some("t1".into())
        );
        assert_eq!(
            extract_tenant_id("/users", Some("limit=10&tenant_id=t2"), None),
            Some("t2".into())
        );
    }

    #[test]
    fn extract_tenant_id_from_body() {
        let body = br#"{"tenant_id":"t3","email":"a@b.com"}"#;
        assert_eq!(
            extract_tenant_id("/users", None, Some(body)),
            Some("t3".into())
        );
    }

    #[test]
    fn path_takes_precedence_over_body() {
        let body = br#"{"tenant_id":"from-body"}"#;
        assert_eq!(
            extract_tenant_id("/tenants/from-path/quotas", None, Some(body)),
            Some("from-path".into())
        );
    }
}
