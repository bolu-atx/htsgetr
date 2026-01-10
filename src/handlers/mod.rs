mod reads;
mod variants;
mod sequences;
mod data;
mod service_info;

pub use reads::{get_reads, post_reads};
pub use variants::{get_variants, post_variants};
pub use sequences::{get_sequences};
pub use data::get_data;
pub use service_info::service_info;

use crate::storage::Storage;
use std::sync::Arc;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<dyn Storage>,
    pub base_url: String,
}
