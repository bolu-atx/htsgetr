use clap::Parser;
use std::path::PathBuf;

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

    /// Base URL for ticket URLs (e.g., https://example.com)
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
}

impl Config {
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
        };
        assert_eq!(config.effective_base_url(), "http://localhost:3000");
    }
}
