use clap::Parser;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use htsgetr::{
    Config,
    config::StorageType,
    handlers::{AppState, create_router},
    storage::{LocalStorage, Storage},
};

#[cfg(feature = "s3")]
use htsgetr::storage::S3Storage;

#[cfg(feature = "http")]
use htsgetr::storage::HttpStorage;

#[cfg(feature = "auth")]
use htsgetr::auth::{AuthConfig, UrlSigner, auth_middleware};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config.log_level.clone().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create storage backend
    let storage: Arc<dyn Storage> = match config.storage {
        StorageType::Local => {
            tracing::info!("Using local storage backend");
            Arc::new(LocalStorage::new(
                config.data_dir.clone(),
                config.effective_base_url(),
            ))
        }
        #[cfg(feature = "s3")]
        StorageType::S3 => {
            let bucket = config
                .s3_bucket
                .clone()
                .ok_or_else(|| anyhow::anyhow!("HTSGET_S3_BUCKET is required for S3 storage"))?;

            tracing::info!("Using S3 storage backend: bucket={}", bucket);

            Arc::new(
                S3Storage::new(
                    bucket,
                    config.s3_prefix.clone(),
                    config.cache_dir.clone(),
                    config.presigned_url_expiry,
                    config.s3_region.clone(),
                    config.s3_endpoint.clone(),
                )
                .await?,
            )
        }
        #[cfg(not(feature = "s3"))]
        StorageType::S3 => {
            anyhow::bail!(
                "S3 storage requires the 's3' feature to be enabled. Rebuild with: cargo build --features s3"
            )
        }
        #[cfg(feature = "http")]
        StorageType::Http => {
            let base_url = config.http_base_url.clone().ok_or_else(|| {
                anyhow::anyhow!("HTSGET_HTTP_BASE_URL is required for HTTP storage")
            })?;

            tracing::info!("Using HTTP storage backend: base_url={}", base_url);

            Arc::new(
                HttpStorage::new(
                    base_url,
                    config.http_index_base_url.clone(),
                    config.cache_dir.clone(),
                )
                .await?,
            )
        }
        #[cfg(not(feature = "http"))]
        StorageType::Http => {
            anyhow::bail!(
                "HTTP storage requires the 'http' feature to be enabled. Rebuild with: cargo build --features http"
            )
        }
    };

    // Create URL signer if auth is enabled
    #[cfg(feature = "auth")]
    let url_signer = if config.auth_enabled {
        let secret = config
            .data_url_secret
            .as_ref()
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_else(|| {
                tracing::info!("Generating random data URL signing secret");
                UrlSigner::generate_secret()
            });
        Some(UrlSigner::new(secret, config.data_url_expiry))
    } else {
        None
    };

    let state = AppState {
        storage,
        base_url: config.effective_base_url(),
        #[cfg(feature = "auth")]
        url_signer: url_signer.clone(),
    };

    // Build router
    let app = create_router(state);

    // Add auth middleware if enabled
    #[cfg(feature = "auth")]
    let app = if config.auth_enabled {
        let auth_config = Arc::new(build_auth_config(&config, url_signer)?);
        // Extension must be added before middleware so middleware can extract it
        app.layer(axum::Extension(auth_config))
            .layer(axum::middleware::from_fn(
                |req: axum::extract::Request, next: axum::middleware::Next| async move {
                    auth_middleware(req, next).await
                },
            ))
    } else {
        app
    };

    let app = app.layer(TraceLayer::new_for_http());

    let app = if config.cors {
        app.layer(CorsLayer::permissive())
    } else {
        app
    };

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting htsgetr server on {}", addr);
    tracing::info!("Data directory: {:?}", config.data_dir);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Build AuthConfig from Config settings.
#[cfg(feature = "auth")]
fn build_auth_config(config: &Config, url_signer: Option<UrlSigner>) -> anyhow::Result<AuthConfig> {
    use htsgetr::auth::{KeyProvider, StaticKeyProvider, jwks::JwksKeyProvider};
    use std::collections::HashSet;

    // Determine key provider
    let key_provider: Arc<dyn KeyProvider> = if let Some(ref pem) = config.auth_public_key {
        // Static PEM key
        tracing::info!("Using static public key for JWT validation");
        Arc::new(StaticKeyProvider::from_rsa_pem(pem.as_bytes())?)
    } else if let Some(ref jwks_url) = config.auth_jwks_url {
        // Explicit JWKS URL
        tracing::info!("Using JWKS endpoint: {}", jwks_url);
        Arc::new(JwksKeyProvider::new(jwks_url.clone()))
    } else if let Some(ref issuer) = config.auth_issuer {
        // Derive JWKS URL from issuer
        tracing::info!("Using JWKS from issuer: {}", issuer);
        Arc::new(JwksKeyProvider::from_issuer(issuer))
    } else {
        anyhow::bail!(
            "Auth enabled but no key source configured. Set HTSGET_AUTH_ISSUER, \
             HTSGET_AUTH_JWKS_URL, or HTSGET_AUTH_PUBLIC_KEY"
        );
    };

    // Parse public endpoints
    let public_paths: HashSet<String> = config
        .auth_public_endpoints
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    tracing::info!("Public endpoints (no auth required): {:?}", public_paths);

    Ok(AuthConfig {
        enabled: true,
        key_provider,
        issuer: config.auth_issuer.clone(),
        audience: config.auth_audience.clone(),
        public_paths,
        url_signer,
    })
}
