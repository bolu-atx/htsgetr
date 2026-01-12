//! HMAC-based URL signing for data endpoints.
//!
//! When authentication is enabled, ticket URLs for `/data/` endpoints are signed
//! with HMAC to prevent unauthorized access without requiring the client to
//! re-authenticate when fetching data blocks.

use crate::Error;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// URL signer using HMAC-SHA256.
#[derive(Clone)]
pub struct UrlSigner {
    secret: Vec<u8>,
    expiry_secs: u64,
}

impl UrlSigner {
    /// Create a new URL signer.
    ///
    /// # Arguments
    /// * `secret` - HMAC secret key
    /// * `expiry_secs` - How long signed URLs are valid (seconds)
    pub fn new(secret: impl Into<Vec<u8>>, expiry_secs: u64) -> Self {
        Self {
            secret: secret.into(),
            expiry_secs,
        }
    }

    /// Generate a random secret key.
    pub fn generate_secret() -> Vec<u8> {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};

        // Use RandomState to generate random bytes
        let state = RandomState::new();
        let mut bytes = Vec::with_capacity(32);
        for _ in 0..4 {
            let hasher = state.build_hasher();
            bytes.extend_from_slice(&hasher.finish().to_le_bytes());
        }
        bytes
    }

    /// Sign a URL with an expiry timestamp.
    ///
    /// Returns the URL with `_expires` and `_sig` query parameters appended.
    pub fn sign_url(&self, url: &str) -> String {
        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs()
            + self.expiry_secs;

        let signature = self.compute_signature(url, expires);

        let separator = if url.contains('?') { '&' } else { '?' };
        format!(
            "{}{}_expires={}&_sig={}",
            url, separator, expires, signature
        )
    }

    /// Validate a signed URL.
    ///
    /// # Arguments
    /// * `base_url` - The URL without signature parameters
    /// * `expires` - The expiry timestamp from `_expires` parameter
    /// * `signature` - The signature from `_sig` parameter
    pub fn validate(&self, base_url: &str, expires: u64, signature: &str) -> Result<(), Error> {
        // Check expiry
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();

        if now > expires {
            tracing::debug!("signed URL expired: now={}, expires={}", now, expires);
            return Err(Error::InvalidAuthentication);
        }

        // Verify signature
        let expected = self.compute_signature(base_url, expires);
        if signature != expected {
            tracing::debug!("invalid URL signature");
            return Err(Error::InvalidAuthentication);
        }

        Ok(())
    }

    /// Compute HMAC signature for a URL and expiry.
    fn compute_signature(&self, url: &str, expires: u64) -> String {
        let message = format!("{}:{}", url, expires);

        let mut mac =
            HmacSha256::new_from_slice(&self.secret).expect("HMAC can take key of any size");
        mac.update(message.as_bytes());

        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }
}

/// Parse signature parameters from a URL.
///
/// Extracts `_expires` and `_sig` query parameters and returns the base URL
/// without these parameters.
pub fn parse_signed_url(url: &str) -> Option<(String, u64, String)> {
    let url_obj = url::Url::parse(url).ok()?;

    let mut expires: Option<u64> = None;
    let mut sig: Option<String> = None;
    let mut base_params = Vec::new();

    for (key, value) in url_obj.query_pairs() {
        match key.as_ref() {
            "_expires" => expires = value.parse().ok(),
            "_sig" => sig = Some(value.to_string()),
            _ => base_params.push((key.to_string(), value.to_string())),
        }
    }

    let expires = expires?;
    let sig = sig?;

    // Reconstruct base URL without signature params
    let mut base_url = format!(
        "{}://{}{}",
        url_obj.scheme(),
        url_obj.host_str().unwrap_or(""),
        url_obj.path()
    );

    if let Some(port) = url_obj.port() {
        base_url = format!(
            "{}://{}:{}{}",
            url_obj.scheme(),
            url_obj.host_str().unwrap_or(""),
            port,
            url_obj.path()
        );
    }

    if !base_params.is_empty() {
        let params: Vec<String> = base_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        base_url = format!("{}?{}", base_url, params.join("&"));
    }

    Some((base_url, expires, sig))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_validate() {
        let signer = UrlSigner::new(b"test-secret".to_vec(), 3600);
        let url = "http://localhost:8080/data/BAM/sample1?start=0&end=1000";

        let signed = signer.sign_url(url);
        assert!(signed.contains("_expires="));
        assert!(signed.contains("_sig="));

        // Parse and validate
        let (base_url, expires, sig) = parse_signed_url(&signed).unwrap();
        assert!(signer.validate(&base_url, expires, &sig).is_ok());
    }

    #[test]
    fn test_expired_url() {
        let signer = UrlSigner::new(b"test-secret".to_vec(), 0);
        let url = "http://localhost:8080/data/BAM/sample1";

        let signed = signer.sign_url(url);

        // Wait for expiry - need to cross a second boundary
        std::thread::sleep(std::time::Duration::from_secs(2));

        let (base_url, expires, sig) = parse_signed_url(&signed).unwrap();
        assert!(signer.validate(&base_url, expires, &sig).is_err());
    }

    #[test]
    fn test_invalid_signature() {
        let signer = UrlSigner::new(b"test-secret".to_vec(), 3600);
        let url = "http://localhost:8080/data/BAM/sample1";

        let signed = signer.sign_url(url);
        let (base_url, expires, _) = parse_signed_url(&signed).unwrap();

        // Use wrong signature
        assert!(signer.validate(&base_url, expires, "wrong-sig").is_err());
    }

    #[test]
    fn test_tampered_url() {
        let signer = UrlSigner::new(b"test-secret".to_vec(), 3600);
        let url = "http://localhost:8080/data/BAM/sample1";

        let signed = signer.sign_url(url);
        let (_, expires, sig) = parse_signed_url(&signed).unwrap();

        // Try to validate with different base URL
        let tampered = "http://localhost:8080/data/BAM/other-sample";
        assert!(signer.validate(tampered, expires, &sig).is_err());
    }

    #[test]
    fn test_parse_signed_url() {
        let url = "http://localhost:8080/data/BAM/sample1?start=0&end=1000&_expires=1234567890&_sig=abc123";
        let (base, expires, sig) = parse_signed_url(url).unwrap();

        assert_eq!(
            base,
            "http://localhost:8080/data/BAM/sample1?start=0&end=1000"
        );
        assert_eq!(expires, 1234567890);
        assert_eq!(sig, "abc123");
    }

    #[test]
    fn test_generate_secret() {
        let secret1 = UrlSigner::generate_secret();
        let secret2 = UrlSigner::generate_secret();

        assert_eq!(secret1.len(), 32);
        assert_eq!(secret2.len(), 32);
        // Secrets should be different (with very high probability)
        assert_ne!(secret1, secret2);
    }
}
