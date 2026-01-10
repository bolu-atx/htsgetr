//! HTTP endpoint handlers for the htsget protocol.
//!
//! This module contains the axum handlers for all htsget endpoints:
//!
//! - [`get_reads`] / [`post_reads`] - `GET/POST /reads/{id}`
//! - [`get_variants`] / [`post_variants`] - `GET/POST /variants/{id}`
//! - [`get_sequences`] - `GET /sequences/{id}` (extension)
//! - [`get_data`] - `GET /data/{format}/{id}` (data serving)
//! - [`service_info`] - `GET /service-info`
//!
//! # Protocol Flow
//!
//! 1. Client calls `/reads/{id}` or `/variants/{id}` with optional region params
//! 2. Server returns a JSON ticket with URLs pointing to `/data/{format}/{id}`
//! 3. Client fetches data blocks from the ticket URLs
//!
//! # Example
//!
//! ```ignore
//! use axum::{Router, routing::get};
//! use htsgetr::handlers::{get_reads, post_reads, AppState};
//!
//! let app = Router::new()
//!     .route("/reads/{id}", get(get_reads).post(post_reads))
//!     .with_state(state);
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
use std::sync::Arc;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<dyn Storage>,
    pub base_url: String,
}
