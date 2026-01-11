//! Python bindings for htsgetr using PyO3
//!
//! Provides a Python interface to run the htsget server and query files directly.

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
use std::path::PathBuf;

#[cfg(feature = "python")]
use std::sync::Arc;

/// Python module for htsgetr
#[cfg(feature = "python")]
#[pymodule]
fn htsgetr(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<HtsgetServer>()?;
    m.add_class::<HtsgetClient>()?;
    Ok(())
}

/// htsget server that can be started from Python
#[cfg(feature = "python")]
#[pyclass]
pub struct HtsgetServer {
    host: String,
    port: u16,
    data_dir: PathBuf,
}

#[cfg(feature = "python")]
#[pymethods]
impl HtsgetServer {
    #[new]
    #[pyo3(signature = (data_dir, host="0.0.0.0".to_string(), port=8080))]
    fn new(data_dir: String, host: String, port: u16) -> Self {
        Self {
            host,
            port,
            data_dir: PathBuf::from(data_dir),
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
        let data_dir = self.data_dir.clone();
        let base_url = format!("http://{}:{}", host, port);

        rt.block_on(async move {
            // Initialize tracing (basic)
            let _ = tracing_subscriber::fmt::try_init();

            // Create storage backend
            let storage = Arc::new(LocalStorage::new(data_dir.clone(), base_url.clone()));

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
            tracing::info!("Data directory: {:?}", data_dir);

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

        response.into_string().map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to read response: {}", e))
        })
    }
}
