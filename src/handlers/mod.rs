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
