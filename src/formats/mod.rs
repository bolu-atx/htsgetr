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
