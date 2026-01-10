use super::IndexedRanges;
use crate::storage::ByteRange;
use crate::types::Region;
use crate::{Error, Result};
use noodles::tabix;
use std::path::Path;

pub struct VcfIndexReader;

impl VcfIndexReader {
    /// Read tabix index and compute byte ranges for given regions
    pub async fn query_ranges(index_path: &Path, _regions: &[Region]) -> Result<IndexedRanges> {
        // Read the tabix index
        let _index = tabix::r#async::read(index_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to read tabix index: {}", e)))?;

        // TODO: Properly compute byte ranges from index
        // Similar to BAM, we need to map reference names and query chunks

        Ok(IndexedRanges {
            header_range: ByteRange {
                start: 0,
                end: Some(65536),
            },
            data_ranges: vec![],
        })
    }

    /// Get header byte range for VCF
    pub async fn header_range(_vcf_path: &Path) -> Result<ByteRange> {
        Ok(ByteRange {
            start: 0,
            end: Some(65536),
        })
    }
}
