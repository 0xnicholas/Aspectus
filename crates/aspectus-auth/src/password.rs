//! Password hashing with argon2id (v0.5.0) and configurable strength policy.

use std::sync::OnceLock;

use argon2::{
    Argon2,
    password_hash::{
        PasswordHash, PasswordHasher as _, PasswordVerifier, SaltString, rand_core::OsRng,
    },
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

/// Configurable password-strength policy.
///
/// Loaded from environment once and exposed via [`validate_password`].
/// All requirements are additive: a password must satisfy every enabled rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordPolicy {
    pub min_length: usize,
    pub require_uppercase: bool,
    pub require_lowercase: bool,
    pub require_digit: bool,
    pub require_special: bool,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 8,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: false,
        }
    }
}

impl PasswordPolicy {
    /// Load policy from environment variables.
    ///
    /// Variables:
    /// - `ASPECTUS_PASSWORD_MIN_LENGTH` (default 8)
    /// - `ASPECTUS_PASSWORD_REQUIRE_UPPERCASE` (default true)
    /// - `ASPECTUS_PASSWORD_REQUIRE_LOWERCASE` (default true)
    /// - `ASPECTUS_PASSWORD_REQUIRE_DIGIT` (default true)
    /// - `ASPECTUS_PASSWORD_REQUIRE_SPECIAL` (default false)
    pub fn from_env() -> Self {
        fn parse_bool(name: &str, default: bool) -> bool {
            std::env::var(name)
                .ok()
                .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on"))
                .unwrap_or(default)
        }

        Self {
            min_length: std::env::var("ASPECTUS_PASSWORD_MIN_LENGTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8),
            require_uppercase: parse_bool("ASPECTUS_PASSWORD_REQUIRE_UPPERCASE", true),
            require_lowercase: parse_bool("ASPECTUS_PASSWORD_REQUIRE_LOWERCASE", true),
            require_digit: parse_bool("ASPECTUS_PASSWORD_REQUIRE_DIGIT", true),
            require_special: parse_bool("ASPECTUS_PASSWORD_REQUIRE_SPECIAL", false),
        }
    }

    /// Validate a password against this policy.
    ///
    /// Returns `Ok(())` if the password is acceptable, otherwise a human-readable
    /// message describing the first failed rule. Callers may collect multiple
    /// violations by calling this repeatedly or extending the error format.
    pub fn validate(&self, password: &str) -> Result<(), &'static str> {
        if password.len() < self.min_length {
            return Err("Password is too short");
        }
        if self.require_uppercase && !password.chars().any(|c| c.is_ascii_uppercase()) {
            return Err("Password must contain an uppercase letter");
        }
        if self.require_lowercase && !password.chars().any(|c| c.is_ascii_lowercase()) {
            return Err("Password must contain a lowercase letter");
        }
        if self.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
            return Err("Password must contain a digit");
        }
        if self.require_special && !password.chars().any(|c| !c.is_ascii_alphanumeric()) {
            return Err("Password must contain a special character");
        }
        Ok(())
    }
}

static PASSWORD_POLICY: OnceLock<PasswordPolicy> = OnceLock::new();

/// Return the globally configured password policy.
///
/// The policy is loaded from environment variables on first access. Tests that
/// need a non-default policy should set the relevant env vars before the first
/// call (or before building the axum app).
pub fn password_policy() -> &'static PasswordPolicy {
    PASSWORD_POLICY.get_or_init(PasswordPolicy::from_env)
}

/// Validate a password using the globally configured policy.
pub fn validate_password(password: &str) -> Result<(), &'static str> {
    password_policy().validate(password)
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

    #[test]
    fn default_policy_requires_mixed_case_and_digit() {
        let policy = PasswordPolicy::default();
        assert!(policy.validate("Hello1world").is_ok());
        assert!(policy.validate("hello1world").is_err()); // no uppercase
        assert!(policy.validate("HELLO1WORLD").is_err()); // no lowercase
        assert!(policy.validate("HelloWorld").is_err()); // no digit
        assert!(policy.validate("Hi1").is_err()); // too short
    }
}
