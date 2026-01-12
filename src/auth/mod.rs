//! Authentication middleware for htsget.
//!
//! This module provides optional JWT/Bearer token authentication with:
//! - JWKS and static public key providers
//! - Path-based public endpoint configuration
//! - HMAC-signed data URLs for ticket fetching
//!
//! Enable with the `auth` feature flag.

mod extractor;
pub mod jwks;
mod jwt;
mod middleware;
mod url_signing;

pub use extractor::{OptionalAuth, RequireAuth};
pub use jwt::Claims;
pub use middleware::auth_middleware;
pub use url_signing::UrlSigner;

use crate::Error;
use std::collections::HashSet;
use std::sync::Arc;

/// Authentication configuration.
#[derive(Clone)]
pub struct AuthConfig {
    /// Whether authentication is enabled.
    pub enabled: bool,
    /// Key provider for JWT validation.
    pub key_provider: Arc<dyn KeyProvider>,
    /// Expected issuer claim.
    pub issuer: Option<String>,
    /// Expected audience claim.
    pub audience: Option<String>,
    /// Paths that don't require authentication.
    pub public_paths: HashSet<String>,
    /// URL signer for data endpoints.
    pub url_signer: Option<UrlSigner>,
}

impl AuthConfig {
    /// Check if a path is public (doesn't require auth).
    pub fn is_public_path(&self, path: &str) -> bool {
        // Exact match
        if self.public_paths.contains(path) {
            return true;
        }

        // Prefix match (for paths like "/api/" that should match "/api/foo")
        self.public_paths.iter().any(|p| {
            // Skip root path for prefix matching - it should only match exactly
            if p == "/" {
                return false;
            }

            if p.ends_with('/') {
                path.starts_with(p)
            } else {
                path.starts_with(&format!("{}/", p))
            }
        })
    }
}

/// Trait for JWT key providers.
#[async_trait::async_trait]
pub trait KeyProvider: Send + Sync {
    /// Get the decoding key, optionally using the key ID from the token header.
    async fn get_key(&self, kid: Option<&str>) -> Result<jsonwebtoken::DecodingKey, Error>;
}

/// Static public key provider (PEM format).
pub struct StaticKeyProvider {
    key: jsonwebtoken::DecodingKey,
}

impl StaticKeyProvider {
    /// Create a new static key provider from a PEM-encoded RSA public key.
    pub fn from_rsa_pem(pem: &[u8]) -> Result<Self, Error> {
        let key = jsonwebtoken::DecodingKey::from_rsa_pem(pem)
            .map_err(|e| Error::Internal(format!("invalid RSA PEM key: {}", e)))?;
        Ok(Self { key })
    }

    /// Create a new static key provider from a PEM-encoded EC public key.
    pub fn from_ec_pem(pem: &[u8]) -> Result<Self, Error> {
        let key = jsonwebtoken::DecodingKey::from_ec_pem(pem)
            .map_err(|e| Error::Internal(format!("invalid EC PEM key: {}", e)))?;
        Ok(Self { key })
    }
}

#[async_trait::async_trait]
impl KeyProvider for StaticKeyProvider {
    async fn get_key(&self, _kid: Option<&str>) -> Result<jsonwebtoken::DecodingKey, Error> {
        Ok(self.key.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_public_path_exact() {
        let config = AuthConfig {
            enabled: true,
            key_provider: Arc::new(MockKeyProvider),
            issuer: None,
            audience: None,
            public_paths: ["/", "/service-info"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            url_signer: None,
        };

        assert!(config.is_public_path("/"));
        assert!(config.is_public_path("/service-info"));
        assert!(!config.is_public_path("/reads/sample1"));
        assert!(!config.is_public_path("/variants/sample1"));
    }

    struct MockKeyProvider;

    #[async_trait::async_trait]
    impl KeyProvider for MockKeyProvider {
        async fn get_key(&self, _kid: Option<&str>) -> Result<jsonwebtoken::DecodingKey, Error> {
            Err(Error::Internal("mock provider".to_string()))
        }
    }
}
