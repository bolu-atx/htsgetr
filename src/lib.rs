//! # htsgetr
//!
//! A Rust implementation of the [htsget protocol](https://samtools.github.io/hts-specs/htsget.html)
//! (v1.3) for serving genomic data over HTTP.
//!
//! ## Overview
//!
//! htsgetr provides an HTTP server that implements the GA4GH htsget protocol, allowing
//! clients to efficiently retrieve slices of genomic data files (BAM, CRAM, VCF, BCF)
//! by genomic region.
//!
//! ## Features
//!
//! - **htsget 1.3 compliant** - Full support for reads and variants endpoints
//! - **Multiple formats** - BAM, CRAM, VCF, BCF via [noodles](https://github.com/zaeleus/noodles)
//! - **Extensions** - FASTA/FASTQ support beyond the spec
//! - **Async** - Built on [tokio](https://tokio.rs) and [axum](https://github.com/tokio-rs/axum)
//! - **Pluggable storage** - Local filesystem and S3 backends
//! - **S3 support** - Presigned URLs, index caching, custom endpoints (MinIO/LocalStack)
//!
//! ## Quick Start
//!
//! ```no_run
//! use htsgetr::Config;
//! use clap::Parser;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = Config::parse();
//!     println!("Starting server on {}:{}", config.host, config.port);
//! }
//! ```
//!
//! ## Architecture
//!
//! The crate is organized into the following modules:
//!
//! - [`config`] - Server configuration and CLI arguments
//! - [`error`] - Error types mapping to htsget protocol errors
//! - [`types`] - Request/response types per the htsget spec
//! - [`handlers`] - HTTP endpoint handlers
//! - [`storage`] - Storage backend abstraction
//! - [`formats`] - Format-specific index readers
//!
//! ## Protocol
//!
//! The htsget protocol works in two phases:
//!
//! 1. **Ticket request** - Client requests data for a sample/region, server returns
//!    a JSON "ticket" containing URLs to fetch the actual data
//! 2. **Data fetch** - Client fetches data blocks from the URLs in the ticket
//!
//! This allows servers to optimize data access patterns and support various
//! storage backends (local, S3, GCS) with presigned URLs.
//!
//! ## Python Bindings
//!
//! htsgetr provides Python bindings via [PyO3](https://pyo3.rs), allowing you to embed
//! an htsget server in Python applications or query htsget endpoints from Python.
//!
//! ### Installation
//!
//! Install from PyPI (when published):
//!
//! ```bash
//! pip install htsgetr
//! ```
//!
//! Or build from source using [maturin](https://github.com/PyO3/maturin):
//!
//! ```bash
//! # Install maturin
//! pip install maturin
//!
//! # Build and install the Python package
//! maturin develop --features python
//! ```
//!
//! ### Example: Running a Server
//!
//! ```python
//! import htsgetr
//!
//! # Create and run a server (blocking)
//! server = htsgetr.HtsgetServer(
//!     data_dir="./data",
//!     host="127.0.0.1",
//!     port=8080
//! )
//! print(f"Server URL: {server.url()}")
//! server.run()  # Blocks until interrupted
//! ```
//!
//! ### Example: Querying an htsget Server
//!
//! ```python
//! import htsgetr
//! import json
//!
//! # Connect to an htsget server
//! client = htsgetr.HtsgetClient("http://localhost:8080")
//!
//! # Fetch reads for a sample (returns JSON ticket)
//! ticket = client.reads("NA12878")
//! print(json.loads(ticket))
//!
//! # Fetch reads for a specific region
//! ticket = client.reads(
//!     "NA12878",
//!     reference_name="chr1",
//!     start=10000,
//!     end=20000
//! )
//!
//! # Fetch variants
//! ticket = client.variants("sample1", reference_name="chr1")
//! ```
//!
//! ### Example: Background Server with Threading
//!
//! ```python
//! import htsgetr
//! import threading
//! import time
//!
//! # Start server in background thread
//! server = htsgetr.HtsgetServer("./data", port=8080)
//! thread = threading.Thread(target=server.run, daemon=True)
//! thread.start()
//!
//! # Give server time to start
//! time.sleep(1)
//!
//! # Now query it
//! client = htsgetr.HtsgetClient(server.url())
//! result = client.reads("sample1")
//! print(result)
//! ```
//!
//! ### Example: S3 Storage Backend
//!
//! ```python
//! import htsgetr
//!
//! # Create server with S3 storage
//! server = htsgetr.HtsgetServer.with_s3(
//!     bucket="my-genomics-bucket",
//!     region="us-west-2",
//!     prefix="samples/",  # Optional key prefix
//!     port=8080
//! )
//! server.run()
//!
//! # For local testing with MinIO or LocalStack
//! server = htsgetr.HtsgetServer.with_s3(
//!     bucket="test-bucket",
//!     endpoint="http://localhost:9000",  # MinIO endpoint
//!     port=8080
//! )
//! ```
//!
//! ### Python API Reference
//!
//! **`HtsgetServer`**
//! - `HtsgetServer(data_dir, host="0.0.0.0", port=8080)` - Create server with local storage
//! - `HtsgetServer.with_s3(bucket, host="0.0.0.0", port=8080, region=None, prefix="", endpoint=None, cache_dir="/tmp/htsgetr-cache", presigned_url_expiry=3600)` - Create server with S3 storage
//! - `server.url()` - Get the server URL
//! - `server.run()` - Start the server (blocking)
//! - `server.is_s3()` - Check if using S3 storage
//!
//! **`HtsgetClient`**
//! - `HtsgetClient(base_url)` - Create a client for an htsget server
//! - `client.reads(id, reference_name=None, start=None, end=None, format=None)` - Fetch reads ticket
//! - `client.variants(id, reference_name=None, start=None, end=None, format=None)` - Fetch variants ticket
//!
//! ## Roadmap
//!
#![doc = include_str!("../docs/roadmap.md")]

pub mod config;
pub mod error;
pub mod formats;
pub mod handlers;
pub mod storage;
pub mod types;

#[cfg(feature = "python")]
pub mod python;

#[cfg(feature = "auth")]
pub mod auth;

pub use config::Config;
pub use error::{Error, Result};
