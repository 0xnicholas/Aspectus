//! Infrastructure integration tests.
//!
//! v0.1.0: Verify testcontainers can start PostgreSQL and Redis.
//! v0.2.0+: Full business logic tests.

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::{postgres, redis};

/// Verify testcontainers can start a PostgreSQL container.
#[tokio::test]
async fn testcontainers_can_start_postgres() {
    let container = postgres::Postgres::default()
        .start()
        .await
        .expect("Failed to start PostgreSQL container");

    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get PostgreSQL port");

    assert!(port > 0, "PostgreSQL should be accessible on a valid port");
}

/// Verify testcontainers can start a Redis container.
#[tokio::test]
async fn testcontainers_can_start_redis() {
    let container = redis::Redis::default()
        .start()
        .await
        .expect("Failed to start Redis container");

    let port = container
        .get_host_port_ipv4(6379)
        .await
        .expect("Failed to get Redis port");

    assert!(port > 0, "Redis should be accessible on a valid port");
}
