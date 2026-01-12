//! HTTP endpoint handlers for the htsget protocol.
//!
//! This module contains the axum handlers for all htsget endpoints:
//!
//! - [`get_reads`] / [`post_reads`] - `GET/POST /reads/:id`
//! - [`get_variants`] / [`post_variants`] - `GET/POST /variants/:id`
//! - [`get_sequences`] - `GET /sequences/:id` (extension)
//! - [`get_data`] - `GET /data/:format/:id` (data serving)
//! - [`service_info()`] - `GET /service-info`
//!
//! # Protocol Flow
//!
//! 1. Client calls `/reads/:id` or `/variants/:id` with optional region params
//! 2. Server returns a JSON ticket with URLs pointing to `/data/:format/:id`
//! 3. Client fetches data blocks from the ticket URLs
//!
//! # Example
//!
//! ```ignore
//! use htsgetr::handlers::{create_router, AppState};
//! use htsgetr::storage::LocalStorage;
//! use std::sync::Arc;
//!
//! let storage = Arc::new(LocalStorage::new(data_dir, base_url.clone()));
//! let state = AppState { storage, base_url };
//! let app = create_router(state);
//! ```

mod data;
mod reads;
mod sequences;
mod service_info;
mod variants;

pub use data::get_data;
pub use reads::{get_reads, post_reads};
pub use sequences::get_sequences;
pub use service_info::service_info;
pub use variants::{get_variants, post_variants};

use crate::storage::Storage;
use axum::{Router, routing::get};
use std::sync::Arc;

#[cfg(feature = "auth")]
use crate::auth::UrlSigner;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<dyn Storage>,
    pub base_url: String,
    /// URL signer for data endpoints (when auth is enabled)
    #[cfg(feature = "auth")]
    pub url_signer: Option<UrlSigner>,
}

impl AppState {
    /// Sign a data URL if authentication is enabled.
    #[cfg(feature = "auth")]
    pub fn sign_data_url(&self, url: String) -> String {
        match &self.url_signer {
            Some(signer) => signer.sign_url(&url),
            None => url,
        }
    }

    /// Sign a data URL (no-op when auth feature is disabled).
    #[cfg(not(feature = "auth"))]
    pub fn sign_data_url(&self, url: String) -> String {
        url
    }
}

/// Create the htsget router with all endpoints configured
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // htsget ticket endpoints
        .route("/reads/:id", get(get_reads).post(post_reads))
        .route("/variants/:id", get(get_variants).post(post_variants))
        .route("/sequences/:id", get(get_sequences))
        // Data serving endpoints (ticket URLs point here)
        .route("/data/:format/:id", get(get_data))
        // Service info
        .route("/", get(service_info))
        .route("/service-info", get(service_info))
        .with_state(state)
}
