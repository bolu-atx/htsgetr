//! Authentication middleware.

use super::{AuthConfig, jwt, url_signing};
use crate::Error;
use axum::{
    body::Body,
    http::{Request, header::AUTHORIZATION},
    middleware::Next,
    response::IntoResponse,
};
use std::sync::Arc;

/// Authentication middleware.
///
/// Checks requests against the auth configuration:
/// - Public paths are allowed without authentication
/// - `/data/` paths require a valid signed URL
/// - All other paths require a valid Bearer token
pub async fn auth_middleware(
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    // Get auth config from extensions
    let auth_config = match request.extensions().get::<Arc<AuthConfig>>() {
        Some(config) => config.clone(),
        None => return next.run(request).await,
    };

    // Skip if auth is disabled
    if !auth_config.enabled {
        return next.run(request).await;
    }

    let path = request.uri().path().to_string();

    // Allow public paths
    if auth_config.is_public_path(&path) {
        return next.run(request).await;
    }

    // Handle /data/ paths with signed URLs
    if path.starts_with("/data/") {
        match validate_signed_data_url(&auth_config, &request) {
            Ok(()) => return next.run(request).await,
            Err(e) => return e.into_response(),
        }
    }

    // All other paths require Bearer token - extract token synchronously
    let token = match extract_bearer_token(&request) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    // Now we can drop the request borrow and do async work
    match validate_jwt_token(&auth_config, token).await {
        Ok(()) => next.run(request).await,
        Err(e) => e.into_response(),
    }
}

/// Extract Bearer token from Authorization header.
fn extract_bearer_token(request: &Request<Body>) -> Result<String, Error> {
    let header = request
        .headers()
        .get(AUTHORIZATION)
        .ok_or(Error::InvalidAuthentication)?;

    let value = header.to_str().map_err(|_| Error::InvalidAuthentication)?;

    value
        .strip_prefix("Bearer ")
        .map(|s| s.to_string())
        .ok_or(Error::InvalidAuthentication)
}

/// Validate a JWT token.
async fn validate_jwt_token(auth_config: &AuthConfig, token: String) -> Result<(), Error> {
    // Decode header to get key ID
    let header = jwt::decode_header(&token)?;
    let kid = header.kid.as_deref();

    // Get the decoding key
    let key = auth_config.key_provider.get_key(kid).await?;

    // Validate the token
    jwt::validate_token(
        &token,
        &key,
        auth_config.issuer.as_deref(),
        auth_config.audience.as_deref(),
    )?;

    Ok(())
}

/// Validate a signed data URL.
fn validate_signed_data_url(
    auth_config: &AuthConfig,
    request: &Request<Body>,
) -> Result<(), Error> {
    let signer = auth_config
        .url_signer
        .as_ref()
        .ok_or(Error::InvalidAuthentication)?;

    // Get the full URI as a string
    let uri = request.uri().to_string();

    // For relative URIs, we need to construct the full URL
    // The signature was computed on the full URL, so we need to match
    let full_url = if uri.starts_with('/') {
        // This is a relative URI, construct full URL
        // We'll use a placeholder host since we just need the path and query
        format!("http://localhost{}", uri)
    } else {
        uri
    };

    let (base_url, expires, sig) = url_signing::parse_signed_url(&full_url).ok_or_else(|| {
        tracing::debug!("missing signature parameters in data URL");
        Error::InvalidAuthentication
    })?;

    signer.validate(&base_url, expires, &sig)
}
