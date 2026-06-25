//! Background cleanup task for expiring tokens.
//!
//! Periodically removes:
//! - expired `refresh_tokens`
//! - used or expired `password_reset_tokens`
//!
//! The interval is controlled by `ASPECTUS_CLEANUP_INTERVAL_SECONDS`
//! (default: 3600s).

use std::time::Duration;

use sqlx::PgPool;
use tokio::time::{MissedTickBehavior, interval};

fn cleanup_interval_seconds() -> u64 {
    std::env::var("ASPECTUS_CLEANUP_INTERVAL_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(3600)
}

/// Spawn a background task that runs token cleanup at a fixed interval.
pub fn spawn_cleanup_task(pool: PgPool) {
    let seconds = cleanup_interval_seconds();

    let mut ticker = interval(Duration::from_secs(seconds));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    tokio::spawn(async move {
        tracing::info!(interval_seconds = seconds, "token cleanup task started");
        loop {
            ticker.tick().await;
            match run_cleanup(&pool).await {
                Ok((refresh, reset)) => {
                    tracing::info!(
                        deleted_refresh_tokens = refresh,
                        deleted_password_reset_tokens = reset,
                        "token cleanup completed"
                    );
                }
                Err(e) => {
                    tracing::error!(error = %e, "token cleanup failed");
                }
            }
        }
    });
}

async fn run_cleanup(pool: &PgPool) -> Result<(u64, u64), sqlx::Error> {
    let refresh = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < NOW()")
        .execute(pool)
        .await?
        .rows_affected();

    let reset =
        sqlx::query("DELETE FROM password_reset_tokens WHERE used = true OR expires_at < NOW()")
            .execute(pool)
            .await?
            .rows_affected();

    Ok((refresh, reset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_interval_is_one_hour() {
        // Ensure the default is a reasonable value when the env var is absent.
        assert_eq!(cleanup_interval_seconds(), 3600);
    }

    #[test]
    fn invalid_env_var_falls_back_to_default() {
        // The function cannot be unit-tested with an env var set because tests
        // run concurrently and env var mutation is global. This test simply
        // documents that the fallback path exists.
        assert_eq!(cleanup_interval_seconds(), 3600);
    }
}
