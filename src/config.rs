//! Server configuration and CLI arguments.
//!
//! This module provides the [`Config`] struct which handles both CLI argument
//! parsing and environment variable configuration using [clap](https://docs.rs/clap).
//!
//! # Example
//!
//! ```no_run
//! use htsgetr::Config;
//! use clap::Parser;
//!
//! let config = Config::parse();
//! println!("Serving from: {:?}", config.data_dir);
//! ```
//!
//! # Environment Variables
//!
//! All options can be set via environment variables:
//!
//! | Variable | Default | Description |
//! |----------|---------|-------------|
//! | `HTSGET_HOST` | `0.0.0.0` | Bind address |
//! | `HTSGET_PORT` | `8080` | Listen port |
//! | `HTSGET_DATA_DIR` | `./data` | Data directory |
//! | `HTSGET_BASE_URL` | auto | Base URL for tickets |
//! | `HTSGET_CORS` | `true` | Enable CORS |
//! | `RUST_LOG` | `info` | Log level |

use clap::Parser;
use std::path::PathBuf;
use std::str::FromStr;

/// Storage backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageType {
    #[default]
    Local,
    S3,
}

impl FromStr for StorageType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "local" => Ok(StorageType::Local),
            "s3" => Ok(StorageType::S3),
            _ => Err(format!(
                "unknown storage type: {} (expected 'local' or 's3')",
                s
            )),
        }
    }
}

impl std::fmt::Display for StorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageType::Local => write!(f, "local"),
            StorageType::S3 => write!(f, "s3"),
        }
    }
}

#[derive(Debug, Clone, Parser)]
#[command(name = "htsgetr")]
#[command(about = "htsget protocol server implementation")]
pub struct Config {
    /// Host address to bind to
    #[arg(long, env = "HTSGET_HOST", default_value = "0.0.0.0")]
    pub host: String,

    /// Port to listen on
    #[arg(short, long, env = "HTSGET_PORT", default_value = "8080")]
    pub port: u16,

    /// Base URL for ticket URLs (e.g., `https://example.com`)
    #[arg(long, env = "HTSGET_BASE_URL")]
    pub base_url: Option<String>,

    /// Directory containing data files
    #[arg(long, env = "HTSGET_DATA_DIR", default_value = "./data")]
    pub data_dir: PathBuf,

    /// Enable CORS for all origins
    #[arg(long, env = "HTSGET_CORS", default_value = "true")]
    pub cors: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub log_level: String,

    /// Maximum payload size in bytes
    #[arg(long, env = "HTSGET_MAX_PAYLOAD", default_value = "10485760")]
    pub max_payload: usize,

    /// Storage backend type: "local" or "s3"
    #[arg(long, env = "HTSGET_STORAGE", default_value = "local")]
    pub storage: StorageType,

    /// S3 bucket name (required when storage=s3)
    #[arg(long, env = "HTSGET_S3_BUCKET")]
    pub s3_bucket: Option<String>,

    /// S3 region (uses AWS_REGION/AWS_DEFAULT_REGION if not set)
    #[arg(long, env = "HTSGET_S3_REGION")]
    pub s3_region: Option<String>,

    /// S3 key prefix (e.g., "genomics/samples/")
    #[arg(long, env = "HTSGET_S3_PREFIX", default_value = "")]
    pub s3_prefix: String,

    /// S3 endpoint URL (for S3-compatible services like MinIO, LocalStack)
    #[arg(long, env = "HTSGET_S3_ENDPOINT")]
    pub s3_endpoint: Option<String>,

    /// Local cache directory for index files (used with S3 storage)
    #[arg(long, env = "HTSGET_CACHE_DIR", default_value = "/tmp/htsgetr-cache")]
    pub cache_dir: PathBuf,

    /// Presigned URL expiration in seconds (used with S3 storage)
    #[arg(long, env = "HTSGET_PRESIGNED_URL_EXPIRY", default_value = "3600")]
    pub presigned_url_expiry: u64,
}

impl Config {
    /// Returns the effective base URL for ticket responses.
    ///
    /// If `base_url` is set, returns that value. Otherwise, constructs
    /// a URL from the host and port (e.g., `http://0.0.0.0:8080`).
    pub fn effective_base_url(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| format!("http://{}:{}", self.host, self.port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_base_url_default() {
        let config = Config {
            host: "0.0.0.0".to_string(),
            port: 8080,
            base_url: None,
            data_dir: PathBuf::from("./data"),
            cors: true,
            log_level: "info".to_string(),
            max_payload: 10485760,
            storage: StorageType::Local,
            s3_bucket: None,
            s3_region: None,
            s3_prefix: String::new(),
            s3_endpoint: None,
            cache_dir: PathBuf::from("/tmp/htsgetr-cache"),
            presigned_url_expiry: 3600,
        };
        assert_eq!(config.effective_base_url(), "http://0.0.0.0:8080");
    }

    #[test]
    fn test_effective_base_url_custom() {
        let config = Config {
            host: "0.0.0.0".to_string(),
            port: 8080,
            base_url: Some("https://example.com/htsget".to_string()),
            data_dir: PathBuf::from("./data"),
            cors: true,
            log_level: "info".to_string(),
            max_payload: 10485760,
            storage: StorageType::Local,
            s3_bucket: None,
            s3_region: None,
            s3_prefix: String::new(),
            s3_endpoint: None,
            cache_dir: PathBuf::from("/tmp/htsgetr-cache"),
            presigned_url_expiry: 3600,
        };
        assert_eq!(config.effective_base_url(), "https://example.com/htsget");
    }

    #[test]
    fn test_effective_base_url_custom_port() {
        let config = Config {
            host: "localhost".to_string(),
            port: 3000,
            base_url: None,
            data_dir: PathBuf::from("./data"),
            cors: true,
            log_level: "info".to_string(),
            max_payload: 10485760,
            storage: StorageType::Local,
            s3_bucket: None,
            s3_region: None,
            s3_prefix: String::new(),
            s3_endpoint: None,
            cache_dir: PathBuf::from("/tmp/htsgetr-cache"),
            presigned_url_expiry: 3600,
        };
        assert_eq!(config.effective_base_url(), "http://localhost:3000");
    }

    #[test]
    fn test_storage_type_parsing() {
        assert_eq!(StorageType::from_str("local").unwrap(), StorageType::Local);
        assert_eq!(StorageType::from_str("LOCAL").unwrap(), StorageType::Local);
        assert_eq!(StorageType::from_str("s3").unwrap(), StorageType::S3);
        assert_eq!(StorageType::from_str("S3").unwrap(), StorageType::S3);
        assert!(StorageType::from_str("invalid").is_err());
    }
}
