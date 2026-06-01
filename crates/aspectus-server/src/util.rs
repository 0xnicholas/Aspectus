/// Generate a random 21-character hex ID for database entities.
///
/// Uses getrandom for cryptographic randomness. Returns an empty string
/// as fallback if RNG fails (never panics).
pub fn generate_id() -> String {
    let mut bytes = [0u8; 16];
    match getrandom::getrandom(&mut bytes) {
        Ok(()) => hex::encode(bytes)[..21].to_string(),
        Err(e) => {
            tracing::error!(error = %e, "RNG failure in generate_id");
            String::new()
        }
    }
}
