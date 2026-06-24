use std::sync::Arc;

use axum::{extract::Request, middleware::Next, response::Response};
use chrono::Utc;
use serde_json::json;

use aspectus_core::{
    audit_log::AuditLog, identity::IdentityType, project::Project, store::AuditLogStore,
};

use crate::util::generate_id;

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
    let actor_project = request.extensions().get::<Project>().copied();
    let audit_store = request
        .extensions()
        .get::<Arc<dyn AuditLogStore>>()
        .cloned();

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

        tokio::spawn(async move {
            let entry = AuditLog {
                id: generate_id(),
                tenant_id: String::new(),
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
