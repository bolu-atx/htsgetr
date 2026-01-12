//! Axum extractors for authenticated users.

use super::{AuthConfig, jwt};
use crate::Error;
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use std::sync::Arc;

/// Authenticated user information extracted from a valid JWT.
#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    /// User subject (sub claim).
    pub subject: Option<String>,
    /// Token issuer (iss claim).
    pub issuer: Option<String>,
}

/// Extractor that requires authentication.
///
/// Returns an error if no valid Bearer token is present.
///
/// # Example
///
/// ```ignore
/// async fn protected_handler(
///     RequireAuth(user): RequireAuth,
/// ) -> impl IntoResponse {
///     format!("Hello, {:?}", user.subject)
/// }
/// ```
pub struct RequireAuth(pub AuthenticatedUser);

/// Extractor for optional authentication.
///
/// Succeeds with `None` if no token is present, or `Some(user)` if valid.
///
/// # Example
///
/// ```ignore
/// async fn optional_auth_handler(
///     OptionalAuth(user): OptionalAuth,
/// ) -> impl IntoResponse {
///     match user {
///         Some(u) => format!("Hello, {:?}", u.subject),
///         None => "Hello, anonymous".to_string(),
///     }
/// }
/// ```
pub struct OptionalAuth(pub Option<AuthenticatedUser>);

impl<S> FromRequestParts<S> for RequireAuth
where
    S: Send + Sync,
{
    type Rejection = Error;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        _state: &'life1 S,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Self, Self::Rejection>>
                + Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            // Get auth config from extensions (set by middleware)
            let auth_config = parts
                .extensions
                .get::<Arc<AuthConfig>>()
                .ok_or(Error::Internal("auth config not found".to_string()))?;

            // Extract Bearer token
            let token = extract_bearer_token(parts)?;

            // Validate token
            let user = validate_token(token, auth_config).await?;

            Ok(RequireAuth(user))
        })
    }
}

impl<S> FromRequestParts<S> for OptionalAuth
where
    S: Send + Sync,
{
    type Rejection = Error;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        _state: &'life1 S,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Self, Self::Rejection>>
                + Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            // Get auth config from extensions
            let auth_config = match parts.extensions.get::<Arc<AuthConfig>>() {
                Some(config) => config,
                None => return Ok(OptionalAuth(None)),
            };

            // Try to extract Bearer token
            let token = match extract_bearer_token(parts) {
                Ok(t) => t,
                Err(_) => return Ok(OptionalAuth(None)),
            };

            // Validate token
            match validate_token(token, auth_config).await {
                Ok(user) => Ok(OptionalAuth(Some(user))),
                Err(_) => Ok(OptionalAuth(None)),
            }
        })
    }
}

/// Extract Bearer token from Authorization header.
fn extract_bearer_token(parts: &Parts) -> Result<&str, Error> {
    let header = parts
        .headers
        .get(AUTHORIZATION)
        .ok_or(Error::InvalidAuthentication)?;

    let value = header.to_str().map_err(|_| Error::InvalidAuthentication)?;

    value
        .strip_prefix("Bearer ")
        .ok_or(Error::InvalidAuthentication)
}

/// Validate a JWT token and return user info.
async fn validate_token(token: &str, auth_config: &AuthConfig) -> Result<AuthenticatedUser, Error> {
    // Decode header to get key ID
    let header = jwt::decode_header(token)?;
    let kid = header.kid.as_deref();

    // Get the decoding key
    let key = auth_config.key_provider.get_key(kid).await?;

    // Validate the token
    let token_data = jwt::validate_token(
        token,
        &key,
        auth_config.issuer.as_deref(),
        auth_config.audience.as_deref(),
    )?;

    Ok(AuthenticatedUser {
        subject: token_data.claims.sub,
        issuer: token_data.claims.iss,
    })
}
