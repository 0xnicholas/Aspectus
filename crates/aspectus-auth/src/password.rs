//! Password hashing with argon2id (v0.5.0).

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher as _, PasswordVerifier, SaltString},
    Argon2,
};

pub struct PasswordHasher;

impl PasswordHasher {
    pub fn hash(password: &str) -> Result<String, String> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| e.to_string())
    }

    pub fn verify(password: &str, hash: &str) -> Result<bool, String> {
        let parsed = PasswordHash::new(hash).map_err(|e| e.to_string())?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map(|_| true)
            .or_else(|e| match e {
                argon2::password_hash::Error::Password => Ok(false),
                other => Err(other.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let hash = PasswordHasher::hash("my-password").unwrap();
        assert!(PasswordHasher::verify("my-password", &hash).unwrap());
        assert!(!PasswordHasher::verify("wrong", &hash).unwrap());
    }

    #[test]
    fn different_salts() {
        let h1 = PasswordHasher::hash("test").unwrap();
        let h2 = PasswordHasher::hash("test").unwrap();
        assert_ne!(h1, h2); // different salts
    }
}
