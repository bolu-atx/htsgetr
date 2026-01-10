//! Storage backend abstraction for genomic data files.
//!
//! This module provides a trait-based abstraction for accessing genomic data files,
//! allowing different storage backends (local filesystem, S3, GCS, etc.) to be used
//! interchangeably.
//!
//! # Implementations
//!
//! - [`LocalStorage`] - Local filesystem storage
//!
//! # Example
//!
//! ```no_run
//! use htsgetr::storage::{Storage, LocalStorage};
//! use htsgetr::types::Format;
//! use std::path::PathBuf;
//!
//! let storage = LocalStorage::new(
//!     PathBuf::from("./data"),
//!     "http://localhost:8080".to_string(),
//! );
//! ```

mod local;

pub use local::LocalStorage;

use crate::{Result, types::Format};
use async_trait::async_trait;
use bytes::Bytes;

/// Byte range within a file
#[derive(Debug, Clone)]
pub struct ByteRange {
    pub start: u64,
    pub end: Option<u64>,
}

/// Metadata about a stored file
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub id: String,
    pub format: Format,
    pub size: u64,
    pub has_index: bool,
}

/// Storage backend trait for accessing genomic data files
#[async_trait]
pub trait Storage: Send + Sync {
    /// Check if a file exists
    async fn exists(&self, id: &str, format: Format) -> Result<bool>;

    /// Get file metadata
    async fn file_info(&self, id: &str, format: Format) -> Result<FileInfo>;

    /// Get URL for accessing a byte range of the file
    /// Returns a URL that can be used to fetch the data
    fn data_url(&self, id: &str, format: Format, range: Option<ByteRange>) -> String;

    /// Read bytes directly (for small inline responses)
    async fn read_bytes(&self, id: &str, format: Format, range: Option<ByteRange>)
    -> Result<Bytes>;

    /// Get index file path if available
    async fn index_path(&self, id: &str, format: Format) -> Result<Option<std::path::PathBuf>>;
}
