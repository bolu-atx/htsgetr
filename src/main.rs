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
            anyhow::bail!("S3 storage requires the 's3' feature to be enabled. Rebuild with: cargo build --features s3")
        }
    };

    let state = AppState {
        storage,
        base_url: config.effective_base_url(),
    };

    // Build router
    let app = create_router(state).layer(TraceLayer::new_for_http());

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
