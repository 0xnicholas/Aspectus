//! JWT signing and verification (v0.4.0).

use anyhow::Context;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
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
    pub aud: String,
    pub iss: String,
    pub iat: usize,
    pub exp: usize,
    pub jti: String,
}

pub struct JwtSigner {
    encoding_key: EncodingKey,
}

impl JwtSigner {
    pub fn from_env() -> anyhow::Result<Self> {
        let pem = std::env::var("JWT_PRIVATE_KEY_PEM")
            .context("JWT_PRIVATE_KEY_PEM required for v0.4 JWT")?;
        Ok(Self { encoding_key: EncodingKey::from_rsa_pem(pem.as_bytes())? })
    }

    pub fn sign(
        &self, sub: &str, tenant_id: &str, project: Project,
        scopes: &str, ttl_seconds: u64,
    ) -> Result<String, CoreError> {
        let now = Utc::now().timestamp() as usize;
        let claims = JwtClaims {
            sub: sub.to_string(), tenant_id: tenant_id.to_string(),
            scope: scopes.to_string(), client_id: project.to_string(),
            aud: project.to_string(), iss: "https://aspectus".into(),
            iat: now, exp: now + ttl_seconds as usize,
            jti: crate::generate_id(),
        };
        encode(&Header::new(jsonwebtoken::Algorithm::RS256), &claims, &self.encoding_key)
            .map_err(|e| CoreError::Internal(format!("JWT: {e}")))
    }
}

pub struct JwtVerifier {
    decoding_key: DecodingKey,
    cache: RedisCache,
}

impl JwtVerifier {
    pub fn from_env(cache: RedisCache) -> anyhow::Result<Self> {
        let pem = std::env::var("JWT_PUBLIC_KEY_PEM")
            .context("JWT_PUBLIC_KEY_PEM required for v0.4 JWT")?;
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
            identity_type: Some(IdentityType::ServiceAccount), client_id: Some(claims.client_id),
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
