use redis::aio::ConnectionManager;
use serde::{de::DeserializeOwned, Serialize};

/// Thin async wrapper around Redis ConnectionManager.
///
/// Clone is cheap (ConnectionManager is Arc-based internally).
#[derive(Clone)]
pub struct RedisCache {
    conn: ConnectionManager,
}

impl RedisCache {
    pub async fn new(client: redis::Client) -> Self {
        Self {
            conn: ConnectionManager::new(client)
                .await
                .expect("Failed to create Redis connection manager"),
        }
    }

    fn conn(&self) -> ConnectionManager {
        self.conn.clone()
    }

    /// Get a JSON-serialized value.
    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let mut conn = self.conn();
        let raw: Option<String> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .ok()?;
        raw.and_then(|s| serde_json::from_str(&s).ok())
    }

    /// Set a JSON-serialized value with TTL (seconds).
    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) {
        let json = serde_json::to_string(value).unwrap();
        let mut conn = self.conn();
        let _: Result<(), _> = redis::cmd("SETEX")
            .arg(key)
            .arg(ttl_secs)
            .arg(&json)
            .query_async(&mut conn)
            .await;
    }

    /// Get a plain Redis string value.
    pub async fn get(&self, key: &str) -> Option<String> {
        let mut conn = self.conn();
        redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .ok()
    }

    /// Set a plain Redis string value with TTL (seconds).
    pub async fn set(&self, key: &str, value: &str, ttl_secs: u64) {
        let mut conn = self.conn();
        let _: Result<(), _> = redis::cmd("SETEX")
            .arg(key)
            .arg(ttl_secs)
            .arg(value)
            .query_async(&mut conn)
            .await;
    }

    /// Delete a key.
    pub async fn del(&self, key: &str) {
        let mut conn = self.conn();
        let _: Result<(), _> = redis::cmd("DEL").arg(key).query_async(&mut conn).await;
    }

    /// Health check — ping the Redis server.
    pub async fn ping(&self) -> Result<(), String> {
        let mut conn = self.conn();
        let result: redis::RedisResult<String> = redis::cmd("PING").query_async(&mut conn).await;
        result.map(|_| ()).map_err(|e| e.to_string())
    }
}
