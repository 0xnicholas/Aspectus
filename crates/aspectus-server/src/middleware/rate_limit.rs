use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use aspectus_auth::RedisCache;

/// Redis-backed fixed-window rate limiter.
///
/// Keys are bucketed by `now / window_secs`, so the limit is enforced per
/// window cluster-wide. This replaces the previous in-memory implementation,
/// which did not share state across replicas.
#[derive(Clone)]
pub struct RateLimiter {
    redis: RedisCache,
    max_requests: usize,
    window_secs: u64,
}

impl RateLimiter {
    pub async fn new(
        redis_client: redis::Client,
        max_requests: usize,
        window_secs: u64,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            redis: RedisCache::new(redis_client).await?,
            max_requests,
            window_secs,
        })
    }

    /// Check if a request under `key` is within the rate limit.
    /// Returns `true` if allowed, `false` if rate-limited.
    pub async fn check(&self, key: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let bucket = now / self.window_secs.max(1);
        let redis_key = format!("aspectus:rate_limit:{}:{}", key, bucket);

        // Atomically increment the counter and set TTL on first hit.
        let script = r#"
            local key = KEYS[1]
            local max = tonumber(ARGV[1])
            local ttl = tonumber(ARGV[2])
            local current = redis.call('INCR', key)
            if current == 1 then
                redis.call('EXPIRE', key, ttl)
            end
            if current > max then
                return 0
            end
            return 1
        "#;

        let allowed: redis::RedisResult<i64> = redis::Script::new(script)
            .key(&redis_key)
            .arg(self.max_requests as i64)
            .arg(self.window_secs as i64)
            .invoke_async(&mut self.redis.conn())
            .await;

        match allowed {
            Ok(1) => true,
            Ok(_) => false,
            Err(e) => {
                // Fail open: if Redis is unavailable, do not block requests.
                // This preserves availability at the cost of temporary rate-limit
                // enforcement. A fail-closed variant can be added later for
                // stricter SLAs.
                tracing::warn!(error = %e, key = %key, "Redis rate limit check failed; allowing request");
                true
            }
        }
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
