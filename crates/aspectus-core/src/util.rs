/// Generate a globally unique, time-sortable identifier using KSUID.
///
/// Returns a 27-character base62-encoded KSUID string suitable for use as a
/// database primary key. KSUIDs embed a 4-byte timestamp (seconds since
/// 2014-05-13, the KSUID epoch) plus 16 bytes of cryptographically random
/// payload, making them roughly sortable by creation time.
///
/// # Panics
///
/// This function never panics. The `ksuid` crate's `generate()` uses the
/// thread-local RNG which is infallible.
pub fn generate_id() -> String {
    ksuid::Ksuid::generate().to_base62()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_id_is_27_chars() {
        let id = generate_id();
        assert_eq!(id.len(), 27, "KSUID base62 is always 27 chars");
    }

    #[test]
    fn generate_id_is_unique() {
        let mut ids = std::collections::HashSet::new();
        for _ in 0..1000 {
            let id = generate_id();
            assert!(ids.insert(id), "KSUID collision detected");
        }
    }

    #[test]
    fn generate_id_parses_back() {
        let id = generate_id();
        let parsed = ksuid::Ksuid::from_base62(&id);
        assert!(parsed.is_ok(), "Generated KSUID should parse back: {id}");
    }
}
