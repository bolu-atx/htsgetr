//! JWKS (JSON Web Key Set) fetching and caching.

use crate::Error;
use jsonwebtoken::DecodingKey;
use moka::future::Cache;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

use super::KeyProvider;

/// JWKS key provider with caching.
pub struct JwksKeyProvider {
    jwks_url: String,
    cache: Cache<String, Arc<Jwks>>,
    http_client: reqwest::Client,
}

impl JwksKeyProvider {
    /// Create a new JWKS key provider.
    ///
    /// # Arguments
    /// * `jwks_url` - URL to fetch JWKS from (e.g., `https://auth.example.com/.well-known/jwks.json`)
    pub fn new(jwks_url: String) -> Self {
        let cache = Cache::builder()
            .time_to_live(Duration::from_secs(3600)) // Cache for 1 hour
            .max_capacity(10)
            .build();

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to create HTTP client");

        Self {
            jwks_url,
            cache,
            http_client,
        }
    }

    /// Create a JWKS key provider from an issuer URL.
    ///
    /// Constructs the JWKS URL as `{issuer}/.well-known/jwks.json`.
    pub fn from_issuer(issuer: &str) -> Self {
        let issuer = issuer.trim_end_matches('/');
        let jwks_url = format!("{}/.well-known/jwks.json", issuer);
        Self::new(jwks_url)
    }

    /// Fetch JWKS from the remote URL.
    async fn fetch_jwks(&self) -> Result<Jwks, Error> {
        tracing::debug!("fetching JWKS from {}", self.jwks_url);

        let response = self
            .http_client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("failed to fetch JWKS: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Internal(format!(
                "JWKS fetch failed with status: {}",
                response.status()
            )));
        }

        response
            .json::<Jwks>()
            .await
            .map_err(|e| Error::Internal(format!("failed to parse JWKS: {}", e)))
    }

    /// Get JWKS, using cache if available.
    async fn get_jwks(&self) -> Result<Arc<Jwks>, Error> {
        const CACHE_KEY: &str = "jwks";

        if let Some(jwks) = self.cache.get(CACHE_KEY).await {
            return Ok(jwks);
        }

        let jwks = Arc::new(self.fetch_jwks().await?);
        self.cache.insert(CACHE_KEY.to_string(), jwks.clone()).await;
        Ok(jwks)
    }
}

#[async_trait::async_trait]
impl KeyProvider for JwksKeyProvider {
    async fn get_key(&self, kid: Option<&str>) -> Result<DecodingKey, Error> {
        let jwks = self.get_jwks().await?;

        let key = match kid {
            Some(kid) => jwks.keys.iter().find(|k| k.kid.as_deref() == Some(kid)),
            None => jwks.keys.first(),
        };

        let key = key.ok_or_else(|| {
            tracing::debug!("no matching key found in JWKS for kid: {:?}", kid);
            Error::InvalidAuthentication
        })?;

        key.to_decoding_key()
    }
}

/// JSON Web Key Set.
#[derive(Debug, Deserialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

/// JSON Web Key.
#[derive(Debug, Deserialize)]
pub struct Jwk {
    /// Key type (e.g., "RSA", "EC").
    pub kty: String,
    /// Key ID.
    pub kid: Option<String>,
    /// Algorithm (e.g., "RS256").
    pub alg: Option<String>,
    /// Key use (e.g., "sig").
    #[serde(rename = "use")]
    pub use_: Option<String>,

    // RSA key components
    /// RSA modulus (base64url).
    pub n: Option<String>,
    /// RSA exponent (base64url).
    pub e: Option<String>,

    // EC key components
    /// EC curve (e.g., "P-256").
    pub crv: Option<String>,
    /// EC x coordinate (base64url).
    pub x: Option<String>,
    /// EC y coordinate (base64url).
    pub y: Option<String>,
}

impl Jwk {
    /// Convert JWK to a DecodingKey.
    pub fn to_decoding_key(&self) -> Result<DecodingKey, Error> {
        match self.kty.as_str() {
            "RSA" => {
                let n = self
                    .n
                    .as_ref()
                    .ok_or_else(|| Error::Internal("RSA key missing 'n'".to_string()))?;
                let e = self
                    .e
                    .as_ref()
                    .ok_or_else(|| Error::Internal("RSA key missing 'e'".to_string()))?;

                DecodingKey::from_rsa_components(n, e)
                    .map_err(|e| Error::Internal(format!("invalid RSA key: {}", e)))
            }
            "EC" => {
                let x = self
                    .x
                    .as_ref()
                    .ok_or_else(|| Error::Internal("EC key missing 'x'".to_string()))?;
                let y = self
                    .y
                    .as_ref()
                    .ok_or_else(|| Error::Internal("EC key missing 'y'".to_string()))?;

                DecodingKey::from_ec_components(x, y)
                    .map_err(|e| Error::Internal(format!("invalid EC key: {}", e)))
            }
            _ => Err(Error::Internal(format!(
                "unsupported key type: {}",
                self.kty
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwk_rsa_missing_components() {
        let jwk = Jwk {
            kty: "RSA".to_string(),
            kid: None,
            alg: None,
            use_: None,
            n: None,
            e: None,
            crv: None,
            x: None,
            y: None,
        };
        assert!(jwk.to_decoding_key().is_err());
    }

    #[test]
    fn test_jwk_unsupported_type() {
        let jwk = Jwk {
            kty: "oct".to_string(),
            kid: None,
            alg: None,
            use_: None,
            n: None,
            e: None,
            crv: None,
            x: None,
            y: None,
        };
        let result = jwk.to_decoding_key();
        assert!(result.is_err());
    }
}
