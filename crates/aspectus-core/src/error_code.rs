/// Machine-readable error codes for stable client-side error handling.
///
/// Each variant maps to a specific problem condition. Clients can branch on
/// these codes without parsing human-readable error messages.
///
/// Serialized as `snake_case` strings matching RFC 7807 `type` URI fragments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // ---- Authentication (401) ----
    /// Invalid email/password combination.
    InvalidCredentials,
    /// The caller's service token is missing, malformed, or revoked.
    InvalidServiceToken,
    /// The user account has been suspended by an administrator.
    AccountSuspended,
    /// OAuth2 client_secret does not match the registered client.
    InvalidClientSecret,
    /// PKCE code_verifier does not match the stored code_challenge.
    InvalidCodeVerifier,
    /// Authorization code has expired or already been consumed.
    InvalidOrExpiredCode,
    /// Refresh token has expired, been revoked, or been rotated.
    InvalidOrExpiredRefreshToken,

    // ---- Authorization (403) ----
    /// Public self-registration is disabled (ASPECTUS_REGISTRATION_ENABLED=false).
    RegistrationDisabled,

    // ---- Not Found (404) ----
    /// The requested tenant does not exist.
    TenantNotFound,
    /// The requested user does not exist.
    UserNotFound,
    /// The requested service account does not exist.
    ServiceAccountNotFound,
    /// The requested API key does not exist or has been revoked.
    ApiKeyNotFound,
    /// The requested role does not exist.
    RoleNotFound,
    /// A generic resource was not found (fallback).
    NotFound,

    // ---- Validation (422) ----
    /// Generic validation failure.
    ValidationFailed,
    /// Email address is syntactically invalid.
    InvalidEmailFormat,
    /// Display name contains disallowed characters or is empty.
    InvalidDisplayName,
    /// Password is shorter than the minimum length (8 chars).
    PasswordTooShort,
    /// The email address is already registered in this tenant.
    EmailAlreadyExists,
    /// The client_id or redirect_uri is not recognized.
    InvalidClientIdOrRedirectUri,
    /// One or more requested scopes are not defined for this project.
    InvalidScope,
    /// A scope string does not conform to the `project:resource:action` format.
    InvalidScopeFormat,
    /// A required token parameter is missing.
    TokenRequired,
    /// The authorization code parameter is missing.
    CodeRequired,
    /// The refresh_token parameter is missing.
    RefreshTokenRequired,
    /// The grant_type is not supported by this endpoint.
    UnsupportedGrantType,
    /// The tenant_id field is required but missing or empty.
    TenantIdRequired,
    /// The tenant name is required but missing, empty, or contains invalid chars.
    TenantNameInvalid,
    /// The role type does not match the assignee type (e.g. user-only role → SA).
    RoleTypeMismatch,
    /// PKCE code_challenge_method must be 'S256' when code_challenge is present.
    InvalidCodeChallengeMethod,
    /// Number of scopes exceeds the per-key limit.
    ScopeExceedsMax,

    // ---- Conflict (409) ----
    /// The requested resource already exists and cannot be duplicated.
    Conflict,

    // ---- Rate Limiting (429) ----
    /// The client has sent too many requests in a given time window.
    TooManyRequests,

    // ---- Internal (500) ----
    /// An unexpected internal error occurred.
    InternalError,
    /// A dependent service (DB, Redis) is temporarily unavailable.
    ServiceUnavailable,
}

impl ErrorCode {
    /// The RFC 7807 `type` URI for this error code.
    pub fn type_uri(self) -> String {
        format!("https://aspectus.dev/errors/{}", self.as_str())
    }

    /// The snake_case string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            // 401
            Self::InvalidCredentials => "invalid_credentials",
            Self::InvalidServiceToken => "invalid_service_token",
            Self::AccountSuspended => "account_suspended",
            Self::InvalidClientSecret => "invalid_client_secret",
            Self::InvalidCodeVerifier => "invalid_code_verifier",
            Self::InvalidOrExpiredCode => "invalid_or_expired_code",
            Self::InvalidOrExpiredRefreshToken => "invalid_or_expired_refresh_token",
            // 403
            Self::RegistrationDisabled => "registration_disabled",
            // 404
            Self::TenantNotFound => "tenant_not_found",
            Self::UserNotFound => "user_not_found",
            Self::ServiceAccountNotFound => "service_account_not_found",
            Self::ApiKeyNotFound => "api_key_not_found",
            Self::RoleNotFound => "role_not_found",
            Self::NotFound => "not_found",
            // 422
            Self::ValidationFailed => "validation_failed",
            Self::InvalidEmailFormat => "invalid_email_format",
            Self::InvalidDisplayName => "invalid_display_name",
            Self::PasswordTooShort => "password_too_short",
            Self::EmailAlreadyExists => "email_already_exists",
            Self::InvalidClientIdOrRedirectUri => "invalid_client_id_or_redirect_uri",
            Self::InvalidScope => "invalid_scope",
            Self::InvalidScopeFormat => "invalid_scope_format",
            Self::TokenRequired => "token_required",
            Self::CodeRequired => "code_required",
            Self::RefreshTokenRequired => "refresh_token_required",
            Self::UnsupportedGrantType => "unsupported_grant_type",
            Self::TenantIdRequired => "tenant_id_required",
            Self::TenantNameInvalid => "tenant_name_invalid",
            Self::RoleTypeMismatch => "role_type_mismatch",
            Self::InvalidCodeChallengeMethod => "invalid_code_challenge_method",
            Self::ScopeExceedsMax => "scope_exceeds_max",
            // 409
            Self::Conflict => "conflict",
            // 429
            Self::TooManyRequests => "too_many_requests",
            // 500
            Self::InternalError => "internal_error",
            Self::ServiceUnavailable => "service_unavailable",
        }
    }

    /// Suggested HTTP status code for this error.
    pub fn http_status(self) -> u16 {
        match self {
            Self::InvalidCredentials
            | Self::InvalidServiceToken
            | Self::AccountSuspended
            | Self::InvalidClientSecret
            | Self::InvalidCodeVerifier
            | Self::InvalidOrExpiredCode
            | Self::InvalidOrExpiredRefreshToken => 401,
            Self::RegistrationDisabled => 403,
            Self::TenantNotFound
            | Self::UserNotFound
            | Self::ServiceAccountNotFound
            | Self::ApiKeyNotFound
            | Self::RoleNotFound
            | Self::NotFound => 404,
            Self::ValidationFailed
            | Self::InvalidEmailFormat
            | Self::InvalidDisplayName
            | Self::PasswordTooShort
            | Self::EmailAlreadyExists
            | Self::InvalidClientIdOrRedirectUri
            | Self::InvalidScope
            | Self::InvalidScopeFormat
            | Self::TokenRequired
            | Self::CodeRequired
            | Self::RefreshTokenRequired
            | Self::UnsupportedGrantType
            | Self::TenantIdRequired
            | Self::TenantNameInvalid
            | Self::RoleTypeMismatch
            | Self::InvalidCodeChallengeMethod
            | Self::ScopeExceedsMax => 422,
            Self::Conflict => 409,
            Self::TooManyRequests => 429,
            Self::InternalError | Self::ServiceUnavailable => 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_error_codes_have_unique_strings() {
        let mut seen = HashSet::new();
        let codes = [
            ErrorCode::InvalidCredentials,
            ErrorCode::InvalidServiceToken,
            ErrorCode::AccountSuspended,
            ErrorCode::InvalidClientSecret,
            ErrorCode::InvalidCodeVerifier,
            ErrorCode::InvalidOrExpiredCode,
            ErrorCode::InvalidOrExpiredRefreshToken,
            ErrorCode::RegistrationDisabled,
            ErrorCode::TenantNotFound,
            ErrorCode::UserNotFound,
            ErrorCode::ServiceAccountNotFound,
            ErrorCode::ApiKeyNotFound,
            ErrorCode::RoleNotFound,
            ErrorCode::NotFound,
            ErrorCode::ValidationFailed,
            ErrorCode::InvalidEmailFormat,
            ErrorCode::InvalidDisplayName,
            ErrorCode::PasswordTooShort,
            ErrorCode::EmailAlreadyExists,
            ErrorCode::InvalidClientIdOrRedirectUri,
            ErrorCode::InvalidScope,
            ErrorCode::InvalidScopeFormat,
            ErrorCode::TokenRequired,
            ErrorCode::CodeRequired,
            ErrorCode::RefreshTokenRequired,
            ErrorCode::UnsupportedGrantType,
            ErrorCode::TenantIdRequired,
            ErrorCode::TenantNameInvalid,
            ErrorCode::RoleTypeMismatch,
            ErrorCode::InvalidCodeChallengeMethod,
            ErrorCode::ScopeExceedsMax,
            ErrorCode::Conflict,
            ErrorCode::TooManyRequests,
            ErrorCode::InternalError,
            ErrorCode::ServiceUnavailable,
        ];
        for code in &codes {
            let s = code.as_str();
            assert!(!s.is_empty(), "ErrorCode must have non-empty string");
            assert!(seen.insert(s), "Duplicate error code string: {s}");
        }
    }

    #[test]
    fn type_uri_contains_code() {
        assert_eq!(
            ErrorCode::UserNotFound.type_uri(),
            "https://aspectus.dev/errors/user_not_found"
        );
        assert_eq!(
            ErrorCode::InvalidCredentials.type_uri(),
            "https://aspectus.dev/errors/invalid_credentials"
        );
    }

    #[test]
    fn http_status_matches_category() {
        assert_eq!(ErrorCode::InvalidCredentials.http_status(), 401);
        assert_eq!(ErrorCode::RegistrationDisabled.http_status(), 403);
        assert_eq!(ErrorCode::UserNotFound.http_status(), 404);
        assert_eq!(ErrorCode::ValidationFailed.http_status(), 422);
        assert_eq!(ErrorCode::TooManyRequests.http_status(), 429);
        assert_eq!(ErrorCode::InternalError.http_status(), 500);
    }
}
