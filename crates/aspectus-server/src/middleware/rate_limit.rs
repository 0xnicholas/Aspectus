use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tokio::sync::Mutex;

/// In-memory sliding-window rate limiter.
///
/// Tracks request timestamps per-key within a sliding window.
/// For multi-replica deployments, replace with a Redis-backed implementation.
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Check if a request under `key` is within the rate limit.
    /// Returns `true` if allowed, `false` if rate-limited.
    async fn check(&self, key: &str) -> bool {
        let mut map = self.inner.lock().await;
        let now = Instant::now();
        let window_start = now - self.window;
        let timestamps = map.entry(key.to_string()).or_default();
        timestamps.retain(|t| *t > window_start);
        if timestamps.len() >= self.max_requests {
            return false;
        }
        timestamps.push(now);
        true
    }
}

/// axum middleware: apply rate limiting using the given key extractor.
pub async fn rate_limit_layer(
    limiter: RateLimiter,
    key_extractor: fn(&Request) -> String,
    req: Request,
    next: Next,
) -> Response {
    let key = key_extractor(&req);
    if !limiter.check(&key).await {
        return build_429_response();
    }
    next.run(req).await
}

/// Extract client IP from request headers (respects X-Forwarded-For).
pub fn ip_key(req: &Request) -> String {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extract service token hash for management API rate limiting.
pub fn service_token_key(req: &Request) -> String {
    req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn build_429_response() -> Response {
    let mut resp = Json(json!({
        "type": "https://aspectus.dev/errors/rate-limited",
        "title": "Too Many Requests",
        "status": 429,
        "detail": "Rate limit exceeded. Please retry after the window expires."
    }))
    .into_response();
    *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
    resp.headers_mut()
        .insert("Retry-After", "60".parse().unwrap());
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        "application/problem+json".parse().unwrap(),
    );
    resp
}
