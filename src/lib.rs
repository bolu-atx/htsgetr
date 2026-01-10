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
//! - **Pluggable storage** - Trait-based storage abstraction
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

pub mod config;
pub mod error;
pub mod formats;
pub mod handlers;
pub mod storage;
pub mod types;

#[cfg(feature = "python")]
pub mod python;

pub use config::Config;
pub use error::{Error, Result};
