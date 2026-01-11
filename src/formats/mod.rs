//! Format-specific index readers using noodles.
//!
//! This module provides readers for genomic index files (BAI, TBI, CSI, CRAI, FAI)
//! that enable efficient byte-range queries for htsget responses.
//!
//! # Supported Formats
//!
//! - [`BamIndexReader`] - BAM index files (`.bai`, `.csi`)
//! - [`VcfIndexReader`] - VCF index files (`.tbi`, `.csi`)
//! - [`BcfIndexReader`] - BCF index files (`.csi`)
//! - [`CramIndexReader`] - CRAM index files (`.crai`)
//! - [`FastaIndexReader`] - FASTA index files (`.fai`)
//! - [`FastqIndexReader`] - FASTQ files (no index, returns whole file)
//!
//! # Index-Based Queries
//!
//! The htsget protocol returns byte ranges that clients can fetch directly.
//! Index readers translate genomic coordinates (chr:start-end) into file
//! byte offsets using the index files.

mod bam;
mod bcf;
mod cram;
mod fasta;
mod fastq;
mod vcf;

pub use bam::BamIndexReader;
pub use bcf::BcfIndexReader;
pub use cram::CramIndexReader;
pub use fasta::FastaIndexReader;
pub use fastq::FastqIndexReader;
pub use vcf::VcfIndexReader;

use crate::storage::ByteRange;

/// Result of querying an index for byte ranges
#[derive(Debug)]
pub struct IndexedRanges {
    pub header_range: ByteRange,
    pub data_ranges: Vec<ByteRange>,
}
