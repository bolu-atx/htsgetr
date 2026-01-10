use axum::{Router, routing::get};
use clap::Parser;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use htsgetr::{
    Config,
    handlers::{
        AppState,
        get_reads, post_reads,
        get_variants, post_variants,
        get_sequences,
        get_data,
        service_info,
    },
    storage::LocalStorage,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| config.log_level.clone().into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create storage backend
    let storage = Arc::new(LocalStorage::new(
        config.data_dir.clone(),
        config.effective_base_url(),
    ));

    let state = AppState {
        storage,
        base_url: config.effective_base_url(),
    };

    // Build router
    let app = Router::new()
        // htsget ticket endpoints
        .route("/reads/{id}", get(get_reads).post(post_reads))
        .route("/variants/{id}", get(get_variants).post(post_variants))
        .route("/sequences/{id}", get(get_sequences))
        // Data serving endpoints (ticket URLs point here)
        .route("/data/{format}/{id}", get(get_data))
        // Service info
        .route("/", get(service_info))
        .route("/service-info", get(service_info))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

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
