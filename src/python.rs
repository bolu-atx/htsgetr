//! Python bindings for htsgetr using PyO3
//!
//! Provides a Python interface to run the htsget server and query files directly.

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
use std::path::PathBuf;

#[cfg(feature = "python")]
use std::sync::Arc;

#[cfg(feature = "python")]
use crate::storage::Storage;

/// Python module for htsgetr
#[cfg(feature = "python")]
#[pymodule]
fn htsgetr(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<HtsgetServer>()?;
    m.add_class::<HtsgetClient>()?;
    Ok(())
}

/// htsget server that can be started from Python
///
/// Supports local filesystem, S3, and HTTP storage backends.
#[cfg(feature = "python")]
#[pyclass]
pub struct HtsgetServer {
    host: String,
    port: u16,
    // Local storage options
    data_dir: Option<PathBuf>,
    // S3 storage options
    s3_bucket: Option<String>,
    s3_region: Option<String>,
    s3_prefix: String,
    s3_endpoint: Option<String>,
    // HTTP storage options
    http_base_url: Option<String>,
    http_index_base_url: Option<String>,
    // Common options
    cache_dir: PathBuf,
    presigned_url_expiry: u64,
}

#[cfg(feature = "python")]
#[pymethods]
impl HtsgetServer {
    /// Create a new htsget server with local storage
    #[new]
    #[pyo3(signature = (data_dir, host="0.0.0.0".to_string(), port=8080))]
    fn new(data_dir: String, host: String, port: u16) -> Self {
        Self {
            host,
            port,
            data_dir: Some(PathBuf::from(data_dir)),
            s3_bucket: None,
            s3_region: None,
            s3_prefix: String::new(),
            s3_endpoint: None,
            http_base_url: None,
            http_index_base_url: None,
            cache_dir: PathBuf::from("/tmp/htsgetr-cache"),
            presigned_url_expiry: 3600,
        }
    }

    /// Create a new htsget server with S3 storage
    #[staticmethod]
    #[pyo3(signature = (bucket, host="0.0.0.0".to_string(), port=8080, region=None, prefix="".to_string(), endpoint=None, cache_dir="/tmp/htsgetr-cache".to_string(), presigned_url_expiry=3600))]
    fn with_s3(
        bucket: String,
        host: String,
        port: u16,
        region: Option<String>,
        prefix: String,
        endpoint: Option<String>,
        cache_dir: String,
        presigned_url_expiry: u64,
    ) -> Self {
        Self {
            host,
            port,
            data_dir: None,
            s3_bucket: Some(bucket),
            s3_region: region,
            s3_prefix: prefix,
            s3_endpoint: endpoint,
            http_base_url: None,
            http_index_base_url: None,
            cache_dir: PathBuf::from(cache_dir),
            presigned_url_expiry,
        }
    }

    /// Create a new htsget server with HTTP storage
    #[staticmethod]
    #[pyo3(signature = (base_url, host="0.0.0.0".to_string(), port=8080, index_base_url=None, cache_dir="/tmp/htsgetr-cache".to_string()))]
    fn with_http(
        base_url: String,
        host: String,
        port: u16,
        index_base_url: Option<String>,
        cache_dir: String,
    ) -> Self {
        Self {
            host,
            port,
            data_dir: None,
            s3_bucket: None,
            s3_region: None,
            s3_prefix: String::new(),
            s3_endpoint: None,
            http_base_url: Some(base_url),
            http_index_base_url: index_base_url,
            cache_dir: PathBuf::from(cache_dir),
            presigned_url_expiry: 3600,
        }
    }

    /// Start the server (blocking)
    fn run(&self) -> PyResult<()> {
        use tower_http::{cors::CorsLayer, trace::TraceLayer};

        use crate::handlers::{AppState, create_router};
        use crate::storage::LocalStorage;

        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let host = self.host.clone();
        let port = self.port;
        let base_url = format!("http://{}:{}", host, port);

        // Clone config for async block
        let data_dir = self.data_dir.clone();
        let s3_bucket = self.s3_bucket.clone();
        let s3_region = self.s3_region.clone();
        let s3_prefix = self.s3_prefix.clone();
        let s3_endpoint = self.s3_endpoint.clone();
        let http_base_url = self.http_base_url.clone();
        let http_index_base_url = self.http_index_base_url.clone();
        let cache_dir = self.cache_dir.clone();
        let presigned_url_expiry = self.presigned_url_expiry;

        rt.block_on(async move {
            // Initialize tracing (basic)
            let _ = tracing_subscriber::fmt::try_init();

            // Create storage backend based on configuration
            let storage: Arc<dyn Storage> = if let Some(bucket) = s3_bucket {
                #[cfg(feature = "s3")]
                {
                    use crate::storage::S3Storage;
                    tracing::info!("Using S3 storage backend: bucket={}", bucket);
                    Arc::new(
                        S3Storage::new(
                            bucket,
                            s3_prefix,
                            cache_dir,
                            presigned_url_expiry,
                            s3_region,
                            s3_endpoint,
                        )
                        .await
                        .map_err(|e| {
                            pyo3::exceptions::PyRuntimeError::new_err(format!(
                                "Failed to create S3 storage: {}",
                                e
                            ))
                        })?,
                    )
                }
                #[cfg(not(feature = "s3"))]
                {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(
                        "S3 storage requires the 's3' feature to be enabled",
                    ));
                }
            } else if let Some(base_url_http) = http_base_url {
                #[cfg(feature = "http")]
                {
                    use crate::storage::HttpStorage;
                    tracing::info!("Using HTTP storage backend: base_url={}", base_url_http);
                    Arc::new(
                        HttpStorage::new(base_url_http, http_index_base_url, cache_dir)
                            .await
                            .map_err(|e| {
                                pyo3::exceptions::PyRuntimeError::new_err(format!(
                                    "Failed to create HTTP storage: {}",
                                    e
                                ))
                            })?,
                    )
                }
                #[cfg(not(feature = "http"))]
                {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(
                        "HTTP storage requires the 'http' feature to be enabled",
                    ));
                }
            } else if let Some(data_dir) = data_dir {
                tracing::info!("Using local storage backend: {:?}", data_dir);
                Arc::new(LocalStorage::new(data_dir, base_url.clone()))
            } else {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "Either data_dir, s3_bucket, or http_base_url must be specified",
                ));
            };

            let state = AppState {
                storage,
                base_url: base_url.clone(),
            };

            // Build router using centralized definition
            let app = create_router(state)
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive());

            let addr = format!("{}:{}", host, port);
            tracing::info!("Starting htsgetr server on {}", addr);

            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

            axum::serve(listener, app)
                .await
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
        })
    }

    /// Get the server URL
    fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    /// Check if this server uses S3 storage
    fn is_s3(&self) -> bool {
        self.s3_bucket.is_some()
    }

    /// Check if this server uses HTTP storage
    fn is_http(&self) -> bool {
        self.http_base_url.is_some()
    }
}

/// Client for making htsget requests
#[cfg(feature = "python")]
#[pyclass]
pub struct HtsgetClient {
    base_url: String,
}

#[cfg(feature = "python")]
#[pymethods]
impl HtsgetClient {
    #[new]
    fn new(base_url: String) -> Self {
        Self { base_url }
    }

    /// Fetch reads for a given ID
    #[pyo3(signature = (id, reference_name=None, start=None, end=None, format=None))]
    fn reads(
        &self,
        id: String,
        reference_name: Option<String>,
        start: Option<u64>,
        end: Option<u64>,
        format: Option<String>,
    ) -> PyResult<String> {
        self.fetch_endpoint("reads", id, reference_name, start, end, format)
    }

    /// Fetch variants for a given ID
    #[pyo3(signature = (id, reference_name=None, start=None, end=None, format=None))]
    fn variants(
        &self,
        id: String,
        reference_name: Option<String>,
        start: Option<u64>,
        end: Option<u64>,
        format: Option<String>,
    ) -> PyResult<String> {
        self.fetch_endpoint("variants", id, reference_name, start, end, format)
    }
}

#[cfg(feature = "python")]
impl HtsgetClient {
    fn fetch_endpoint(
        &self,
        endpoint: &str,
        id: String,
        reference_name: Option<String>,
        start: Option<u64>,
        end: Option<u64>,
        format: Option<String>,
    ) -> PyResult<String> {
        // Build URL with query parameters
        let mut url = format!("{}/{}/{}", self.base_url, endpoint, id);
        let mut params = Vec::new();

        if let Some(fmt) = format {
            params.push(format!("format={}", fmt));
        }
        if let Some(ref_name) = reference_name {
            params.push(format!("referenceName={}", ref_name));
        }
        if let Some(s) = start {
            params.push(format!("start={}", s));
        }
        if let Some(e) = end {
            params.push(format!("end={}", e));
        }

        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }

        // Make HTTP request using ureq
        let response = ureq::get(&url).call().map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("HTTP request failed: {}", e))
        })?;

        response.into_body().read_to_string().map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to read response: {}", e))
        })
    }
}
