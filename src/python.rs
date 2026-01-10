//! Python bindings for htsgetr using PyO3
//!
//! Provides a Python interface to run the htsget server and query files directly.

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
use std::path::PathBuf;

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
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let host = self.host.clone();
        let port = self.port;
        let data_dir = self.data_dir.clone();

        rt.block_on(async move {
            // TODO: Implement server startup
            tracing::info!("Starting server on {}:{}", host, port);
            tracing::info!("Data directory: {:?}", data_dir);
            Ok(())
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
        // TODO: Implement actual HTTP request
        Ok(format!(
            "{{\"htsget\": {{\"format\": \"{}\", \"urls\": []}}}}",
            format.unwrap_or_else(|| "BAM".to_string())
        ))
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
        // TODO: Implement actual HTTP request
        Ok(format!(
            "{{\"htsget\": {{\"format\": \"{}\", \"urls\": []}}}}",
            format.unwrap_or_else(|| "VCF".to_string())
        ))
    }
}
