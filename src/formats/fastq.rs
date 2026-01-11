use super::IndexedRanges;
use crate::storage::ByteRange;
use crate::types::Region;
use crate::{Error, Result};
use std::path::Path;
use tokio::fs;

/// FASTQ format reader.
///
/// FASTQ files do not have a standard index format, so all queries
/// return the whole file. This is valid htsget behavior - servers
/// may return supersets of requested data.
pub struct FastqIndexReader;

impl FastqIndexReader {
    /// FASTQ files have no index - always return whole file.
    /// Region parameters are accepted but ignored.
    pub async fn query_ranges(fastq_path: &Path, _regions: &[Region]) -> Result<IndexedRanges> {
        let metadata = fs::metadata(fastq_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to read FASTQ metadata: {}", e)))?;

        // FASTQ has no header/body distinction in htsget sense
        // Return empty header and whole file as single data range
        Ok(IndexedRanges {
            header_range: ByteRange {
                start: 0,
                end: Some(0),
            },
            data_ranges: vec![ByteRange {
                start: 0,
                end: Some(metadata.len()),
            }],
        })
    }

    /// FASTQ files have no header in htsget sense - return empty range
    pub async fn header_range(_fastq_path: &Path) -> Result<ByteRange> {
        Ok(ByteRange {
            start: 0,
            end: Some(0),
        })
    }

    /// Get whole file as the only data range
    pub async fn whole_file_range(fastq_path: &Path) -> Result<ByteRange> {
        let metadata = fs::metadata(fastq_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to read FASTQ metadata: {}", e)))?;

        Ok(ByteRange {
            start: 0,
            end: Some(metadata.len()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fastq_header_range() {
        // FASTQ has no header - should return empty range
        let range = FastqIndexReader::header_range(Path::new("nonexistent.fq")).await;
        assert!(range.is_ok());
        let range = range.unwrap();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, Some(0));
    }
}
