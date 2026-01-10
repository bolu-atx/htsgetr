//! Format-specific index readers using noodles.
//!
//! This module provides readers for genomic index files (BAI, TBI, CSI)
//! that enable efficient byte-range queries for htsget responses.
//!
//! # Supported Formats
//!
//! - [`BamIndexReader`] - BAM index files (`.bai`, `.csi`)
//! - [`VcfIndexReader`] - VCF/BCF index files (`.tbi`, `.csi`)
//!
//! # Index-Based Queries
//!
//! The htsget protocol returns byte ranges that clients can fetch directly.
//! Index readers translate genomic coordinates (chr:start-end) into file
//! byte offsets using the index files.

mod bam;
mod vcf;

pub use bam::BamIndexReader;
pub use vcf::VcfIndexReader;

use crate::storage::ByteRange;

/// Result of querying an index for byte ranges
#[derive(Debug)]
pub struct IndexedRanges {
    pub header_range: ByteRange,
    pub data_ranges: Vec<ByteRange>,
}
