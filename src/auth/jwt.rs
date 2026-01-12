//! JWT validation logic.

use crate::Error;
use jsonwebtoken::{Algorithm, DecodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};

/// Standard JWT claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID).
    pub sub: Option<String>,
    /// Issuer.
    pub iss: Option<String>,
    /// Audience.
    pub aud: Option<Aud>,
    /// Expiration time (Unix timestamp).
    pub exp: Option<u64>,
    /// Issued at (Unix timestamp).
    pub iat: Option<u64>,
    /// Not before (Unix timestamp).
    pub nbf: Option<u64>,
}

/// Audience can be a single string or array of strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Aud {
    Single(String),
    Multiple(Vec<String>),
}

impl Aud {
    /// Check if the audience contains a specific value.
    pub fn contains(&self, value: &str) -> bool {
        match self {
            Aud::Single(s) => s == value,
            Aud::Multiple(v) => v.iter().any(|s| s == value),
        }
    }
}

/// Decode a JWT header without validation to extract the key ID.
pub fn decode_header(token: &str) -> Result<Header, Error> {
    jsonwebtoken::decode_header(token).map_err(|e| {
        tracing::debug!("failed to decode JWT header: {}", e);
        Error::InvalidAuthentication
    })
}

/// Validate a JWT token and extract claims.
pub fn validate_token(
    token: &str,
    key: &DecodingKey,
    issuer: Option<&str>,
    audience: Option<&str>,
) -> Result<TokenData<Claims>, Error> {
    let mut validation = Validation::new(Algorithm::RS256);

    // Also allow ES256 for EC keys
    validation.algorithms = vec![Algorithm::RS256, Algorithm::ES256];

    // Configure issuer validation
    if let Some(iss) = issuer {
        validation.set_issuer(&[iss]);
    } else {
        validation.iss = None;
    }

    // Configure audience validation
    if let Some(aud) = audience {
        validation.set_audience(&[aud]);
    } else {
        validation.aud = None;
    }

    jsonwebtoken::decode::<Claims>(token, key, &validation).map_err(|e| {
        tracing::debug!("JWT validation failed: {}", e);
        match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                tracing::debug!("token expired");
                Error::InvalidAuthentication
            }
            jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                tracing::debug!("invalid issuer");
                Error::InvalidAuthentication
            }
            jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                tracing::debug!("invalid audience");
                Error::InvalidAuthentication
            }
            _ => Error::InvalidAuthentication,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aud_single() {
        let aud = Aud::Single("api".to_string());
        assert!(aud.contains("api"));
        assert!(!aud.contains("other"));
    }

    #[test]
    fn test_aud_multiple() {
        let aud = Aud::Multiple(vec!["api".to_string(), "web".to_string()]);
        assert!(aud.contains("api"));
        assert!(aud.contains("web"));
        assert!(!aud.contains("other"));
    }

    #[test]
    fn test_decode_header_invalid() {
        let result = decode_header("not-a-valid-jwt");
        assert!(result.is_err());
    }
}
