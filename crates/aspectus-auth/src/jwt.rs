//! JWT signing and verification (v0.4.0).

use std::sync::OnceLock;

use base64::Engine;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rsa::pkcs8::DecodePrivateKey;
use rsa::traits::PublicKeyParts;
use serde::{Deserialize, Serialize};

use aspectus_core::{error::CoreError, identity::IdentityType, introspect::IntrospectResponse, project::Project};

use crate::cache::RedisCache;

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub tenant_id: String,
    pub scope: String,
    #[serde(rename = "client_id")]
    pub client_id: String,
    #[serde(rename = "identity_type")]
    pub identity_type: String,
    pub aud: String,
    pub iss: String,
    pub iat: usize,
    pub exp: usize,
    pub jti: String,
}

/// Computed once from the private key at startup.
fn build_jwks(pem: &str) -> serde_json::Value {
    let private_key = rsa::RsaPrivateKey::from_pkcs8_pem(pem)
        .expect("Invalid JWT private key PEM");
    let public_key = private_key.to_public_key();
    let n = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
    let e = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());
    // Use SHA256 of the PEM modulus as a stable key ID
    let kid = {
        use sha2::Digest;
        let hash = sha2::Sha256::digest(public_key.n().to_bytes_be());
        hex::encode(&hash[..8])
    };
    serde_json::json!({
        "keys": [{
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": kid,
            "n": n,
            "e": e
        }]
    })
}

pub struct JwtSigner {
    encoding_key: EncodingKey,
    jwks: OnceLock<serde_json::Value>,
}

const TEST_PRIVATE: &str = include_str!("test_private.pem");
const TEST_PUBLIC: &str = include_str!("test_public.pem");

impl JwtSigner {
    pub fn from_env() -> anyhow::Result<Self> {
        let pem = std::env::var("JWT_PRIVATE_KEY_PEM")
            .ok()
            .and_then(|v| {
                // Try as file path first, then as inline PEM
                if std::path::Path::new(&v).exists() {
                    std::fs::read_to_string(&v).ok()
                } else {
                    Some(v)
                }
            })
            .unwrap_or_else(|| {
                tracing::info!("Using dev test JWT keys");
                TEST_PRIVATE.to_string()
            });
        let jwks = build_jwks(&pem);
        tracing::info!(kid = %jwks["keys"][0]["kid"], "JWKS ready");
        Ok(Self { encoding_key: EncodingKey::from_rsa_pem(pem.as_bytes())?, jwks: OnceLock::from(jwks) })
    }
    pub fn sign(
        &self, sub: &str, tenant_id: &str, project: Project,
        scopes: &str, identity_type: IdentityType, ttl_seconds: u64,
    ) -> Result<String, CoreError> {
        let now = Utc::now().timestamp() as usize;
        let it: &str = identity_type.into();
        let claims = JwtClaims {
            sub: sub.to_string(), tenant_id: tenant_id.to_string(),
            scope: scopes.to_string(), client_id: project.to_string(),
            identity_type: it.to_string(),
            aud: project.to_string(), iss: "https://aspectus".into(),
            iat: now, exp: now + ttl_seconds as usize,
            jti: crate::generate_id(),
        };
        encode(&Header::new(jsonwebtoken::Algorithm::RS256), &claims, &self.encoding_key)
            .map_err(|e| CoreError::Internal(format!("JWT: {e}")))
    }
    pub fn jwks_json(&self) -> serde_json::Value {
        self.jwks.get().cloned().expect("JWKS not initialized")
    }
}

pub struct JwtVerifier {
    decoding_key: DecodingKey,
    cache: RedisCache,
}

impl JwtVerifier {
    pub fn from_env(cache: RedisCache) -> anyhow::Result<Self> {
        let pem = std::env::var("JWT_PUBLIC_KEY_PEM")
            .ok()
            .and_then(|v| {
                if std::path::Path::new(&v).exists() {
                    std::fs::read_to_string(&v).ok()
                } else {
                    Some(v)
                }
            })
            .unwrap_or_else(|| TEST_PUBLIC.to_string());
        Ok(Self { decoding_key: DecodingKey::from_rsa_pem(pem.as_bytes())?, cache })
    }
    pub async fn verify(&self, token: &str) -> IntrospectResponse {
        let mut v = Validation::new(jsonwebtoken::Algorithm::RS256);
        v.validate_exp = true;
        let claims = match decode::<JwtClaims>(token, &self.decoding_key, &v) {
            Ok(d) => d.claims, Err(_) => return IntrospectResponse::inactive(),
        };
        if self.cache.get(&format!("jwt_revoked:{}", claims.jti)).await.is_some() {
            return IntrospectResponse::inactive();
        }
        IntrospectResponse {
            active: true, tenant_id: Some(claims.tenant_id), user_id: Some(claims.sub),
            identity_type: Some(claims.identity_type.as_str().try_into().unwrap_or(IdentityType::User)),
            client_id: Some(claims.client_id),
            scope: Some(claims.scope), token_type: Some("Bearer".into()),
            exp: Some(claims.exp as i64), quotas: None, token_format: Some("jwt".into()),
        }
    }
    pub async fn revoke(&self, token: &str) -> bool {
        let mut v = Validation::new(jsonwebtoken::Algorithm::RS256);
        v.validate_exp = false;
        v.insecure_disable_signature_validation();
        let claims = match decode::<JwtClaims>(token, &self.decoding_key, &v) {
            Ok(d) => d.claims, Err(_) => return false,
        };
        let ttl = claims.exp.saturating_sub(Utc::now().timestamp() as usize).max(1) as u64;
        self.cache.set(&format!("jwt_revoked:{}", claims.jti), "1", ttl).await;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwks_generation_produces_valid_jwk() {
        let jwks = build_jwks(TEST_PRIVATE);
        let keys = jwks["keys"].as_array().unwrap();
        assert_eq!(keys.len(), 1);

        let key = &keys[0];
        assert_eq!(key["kty"], "RSA");
        assert_eq!(key["use"], "sig");
        assert_eq!(key["alg"], "RS256");
        assert!(key["kid"].as_str().unwrap().len() == 16); // hex of 8 bytes
        assert!(!key["n"].as_str().unwrap().is_empty());
        assert!(!key["e"].as_str().unwrap().is_empty());
    }

    #[test]
    fn jwks_is_deterministic() {
        let jwks1 = build_jwks(TEST_PRIVATE);
        let jwks2 = build_jwks(TEST_PRIVATE);
        assert_eq!(jwks1.to_string(), jwks2.to_string());
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let signer = JwtSigner::from_env().expect("JWT signer");
        let token = signer.sign(
            "user-1", "tenant-1", Project::Pandaria,
            "pandaria:session:create pandaria:session:read",
            IdentityType::User, 900,
        ).expect("sign");
        assert!(token.starts_with("eyJ"));

        // Decode and verify claims
        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.validate_exp = false;
        validation.validate_aud = false;
        let jwks = build_jwks(TEST_PRIVATE);
        let key = &jwks["keys"][0];
        let decoding_key = DecodingKey::from_rsa_components(
            key["n"].as_str().unwrap(),
            key["e"].as_str().unwrap(),
        ).expect("decoding key");

        let data = decode::<JwtClaims>(&token, &decoding_key, &validation).expect("decode");
        assert_eq!(data.claims.sub, "user-1");
        assert_eq!(data.claims.tenant_id, "tenant-1");
        assert_eq!(data.claims.identity_type, "user");
        assert!(data.claims.scope.contains("pandaria:session:create"));
    }

    #[test]
    fn jwks_key_can_verify_signed_token() {
        let signer = JwtSigner::from_env().expect("JWT signer");
        let token = signer.sign(
            "sa-1", "t1", Project::Tavern,
            "tavern:workflow:run",
            IdentityType::ServiceAccount, 3600,
        ).expect("sign");

        let jwks = signer.jwks_json();
        let key = &jwks["keys"][0];
        let decoding_key = DecodingKey::from_rsa_components(
            key["n"].as_str().unwrap(),
            key["e"].as_str().unwrap(),
        ).expect("decoding key");

        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.validate_exp = false;
        validation.validate_aud = false;
        let data = decode::<JwtClaims>(&token, &decoding_key, &validation).expect("decode");
        assert_eq!(data.claims.identity_type, "service_account");
    }
}
