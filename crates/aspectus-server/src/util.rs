/// Generate a random 21-character hex ID for database entities.
///
/// Uses getrandom for cryptographic randomness. Returns a fallback
/// timestamp-based ID if RNG fails (never panics).
///
/// NOTE: All ID columns are varchar(21). KSUID upgrade requires
/// first widening columns to varchar(27) via migration.
pub fn generate_id() -> String {
    let mut bytes = [0u8; 16];
    match getrandom::getrandom(&mut bytes) {
        Ok(()) => hex::encode(bytes)[..21].to_string(),
        Err(e) => {
            tracing::error!(error = %e, "RNG failure in generate_id — using fallback");
            // Timestamp-based fallback: still unique within a process
            use std::time::{SystemTime, UNIX_EPOCH};
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            format!("{ts:021}")
        }
    }
}
