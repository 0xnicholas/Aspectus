use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Json,
    extract::Request,
    extract::connect_info::ConnectInfo,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use ipnetwork::IpNetwork;
use serde_json::json;
use sha2::{Digest, Sha256};

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

/// Parse `ASPECTUS_TRUSTED_PROXIES` (comma-separated CIDRs) once per call.
/// Production setups should set this to the ingress/reverse-proxy CIDRs.
fn trusted_proxy_networks() -> Vec<IpNetwork> {
    std::env::var("ASPECTUS_TRUSTED_PROXIES")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

fn is_trusted_proxy(addr: &SocketAddr) -> bool {
    trusted_proxy_networks()
        .iter()
        .any(|net| net.contains(addr.ip()))
}

/// Extract client IP for rate limiting.
///
/// - Uses `X-Forwarded-For` only when the immediate upstream (`ConnectInfo`)
///   is in `ASPECTUS_TRUSTED_PROXIES`.
/// - Otherwise falls back to the direct connection IP.
/// - Returns `"unknown"` when no IP can be determined.
pub fn ip_key(req: &Request) -> String {
    let forwarded = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string());

    let remote = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0);

    match (remote, forwarded) {
        (Some(remote), Some(forwarded)) if is_trusted_proxy(&remote) => forwarded,
        (Some(remote), _) => remote.ip().to_string(),
        (None, Some(forwarded)) => forwarded,
        _ => "unknown".to_string(),
    }
}

/// Extract service token hash for management API rate limiting.
///
/// Uses a SHA-256 hash of the token instead of the token itself so that
/// Redis does not store plaintext internal credentials.
pub fn service_token_key(req: &Request) -> String {
    req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| format!("st:{}", hex::encode(Sha256::digest(s.as_bytes()))))
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

#[cfg(test)]
mod tests {
    use axum::body::Body;

    use super::*;

    #[test]
    fn service_token_key_uses_hash_not_plaintext() {
        let token = "aspectus-dev-secret-token";
        let req = Request::builder()
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let key = service_token_key(&req);
        assert!(
            !key.contains(token),
            "rate-limit key must not contain the plaintext service token"
        );
        assert!(key.starts_with("st:"));
        let expected_hash = format!("st:{}", hex::encode(Sha256::digest(token.as_bytes())));
        assert_eq!(key, expected_hash);
    }

    #[test]
    fn service_token_key_without_authorization_is_unknown() {
        let req = Request::builder().body(Body::empty()).unwrap();
        assert_eq!(service_token_key(&req), "unknown");
    }

    #[test]
    fn ip_key_prefers_x_forwarded_for() {
        let req = Request::builder()
            .header("x-forwarded-for", "1.2.3.4, 5.6.7.8")
            .body(Body::empty())
            .unwrap();
        assert_eq!(ip_key(&req), "1.2.3.4");
    }

    #[test]
    fn ip_key_defaults_to_unknown() {
        let req = Request::builder().body(Body::empty()).unwrap();
        assert_eq!(ip_key(&req), "unknown");
    }

    fn request_with_connect_info(remote: &str, xff: Option<&str>) -> Request {
        let mut req = if let Some(xff) = xff {
            Request::builder().header("x-forwarded-for", xff)
        } else {
            Request::builder()
        }
        .body(Body::empty())
        .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(remote.parse::<SocketAddr>().unwrap()));
        req
    }

    #[test]
    fn ip_key_uses_x_forwarded_for_when_trusted_proxy() {
        unsafe {
            std::env::set_var("ASPECTUS_TRUSTED_PROXIES", "127.0.0.1/32");
        }
        let req = request_with_connect_info("127.0.0.1:12345", Some("10.0.0.1, 10.0.0.2"));
        assert_eq!(ip_key(&req), "10.0.0.1");
    }

    #[test]
    fn ip_key_ignores_x_forwarded_for_when_untrusted_proxy() {
        unsafe {
            std::env::set_var("ASPECTUS_TRUSTED_PROXIES", "127.0.0.1/32");
        }
        let req = request_with_connect_info("192.168.1.1:12345", Some("10.0.0.1"));
        assert_eq!(ip_key(&req), "192.168.1.1");
    }

    #[test]
    fn ip_key_falls_back_to_remote_ip() {
        unsafe {
            std::env::set_var("ASPECTUS_TRUSTED_PROXIES", "");
        }
        let req = request_with_connect_info("10.0.0.5:12345", None);
        assert_eq!(ip_key(&req), "10.0.0.5");
    }
}
