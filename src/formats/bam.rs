use super::IndexedRanges;
use crate::storage::ByteRange;
use crate::types::Region;
use crate::{Error, Result};
use noodles::bam;
use noodles::bam::bai;
use noodles::core::Position;
use noodles::core::region::Interval;
use noodles::csi::binning_index::BinningIndex;
use noodles::csi::binning_index::index::reference_sequence::bin::Chunk;
use noodles::sam;
use std::path::Path;
use tokio::fs::File;

pub struct BamIndexReader;

impl BamIndexReader {
    /// Read BAI/CSI index and compute byte ranges for given regions
    pub async fn query_ranges(
        bam_path: &Path,
        index_path: &Path,
        regions: &[Region],
        header: &sam::Header,
    ) -> Result<IndexedRanges> {
        // Read the BAI index
        let index = bai::r#async::read(index_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to read BAI index: {}", e)))?;

        // Compute header byte range
        let header_range = Self::header_range(bam_path).await?;

        // If no regions specified, return empty data_ranges (caller should serve whole file)
        if regions.is_empty() {
            return Ok(IndexedRanges {
                header_range,
                data_ranges: vec![],
            });
        }

        // Query index for each region
        let mut chunks: Vec<Chunk> = Vec::new();

        for region in regions {
            // Map reference name to reference sequence ID
            let ref_id = header
                .reference_sequences()
                .get_index_of(region.reference_name.as_bytes())
                .ok_or_else(|| {
                    Error::NotFound(format!(
                        "reference sequence not found: {}",
                        region.reference_name
                    ))
                })?;

            // Build interval from region coordinates
            // htsget uses 0-based half-open coordinates, noodles uses 1-based closed
            let start = region
                .start
                .map(|s| Position::try_from(s as usize + 1))
                .transpose()
                .map_err(|e| Error::InvalidRange(format!("invalid start position: {}", e)))?
                .unwrap_or(Position::MIN);

            let end = region
                .end
                .map(|e| Position::try_from(e as usize))
                .transpose()
                .map_err(|e| Error::InvalidRange(format!("invalid end position: {}", e)))?
                .unwrap_or(Position::MAX);

            let interval = Interval::from(start..=end);

            // Query the index for chunks overlapping this region
            let region_chunks = index
                .query(ref_id, interval)
                .map_err(|e| Error::Internal(format!("index query failed: {}", e)))?;

            chunks.extend(region_chunks);
        }

        // Convert chunks to byte ranges
        let mut data_ranges: Vec<ByteRange> = chunks
            .into_iter()
            .map(|chunk| ByteRange {
                start: chunk.start().compressed(),
                end: Some(chunk.end().compressed()),
            })
            .collect();

        // Merge overlapping/adjacent ranges for efficiency
        data_ranges = Self::merge_ranges(data_ranges);

        Ok(IndexedRanges {
            header_range,
            data_ranges,
        })
    }

    /// Compute the header byte range by reading the BAM file
    pub async fn header_range(bam_path: &Path) -> Result<ByteRange> {
        let file = File::open(bam_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to open BAM file: {}", e)))?;

        // bam::Reader::new wraps the file in a BGZF reader internally - don't double-wrap
        let mut reader = bam::r#async::io::Reader::new(file);

        // Read header to advance position past it
        reader
            .read_header()
            .await
            .map_err(|e| Error::Internal(format!("failed to read BAM header: {}", e)))?;

        // Get the virtual position after the header
        let header_end = reader.get_ref().virtual_position();

        Ok(ByteRange {
            start: 0,
            end: Some(header_end.compressed()),
        })
    }

    /// Read the BAM header from a file
    pub async fn read_header(bam_path: &Path) -> Result<sam::Header> {
        let file = File::open(bam_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to open BAM file: {}", e)))?;

        // bam::Reader::new wraps the file in a BGZF reader internally - don't double-wrap
        let mut reader = bam::r#async::io::Reader::new(file);

        reader
            .read_header()
            .await
            .map_err(|e| Error::Internal(format!("failed to read BAM header: {}", e)))
    }

    /// Merge overlapping or adjacent byte ranges
    fn merge_ranges(mut ranges: Vec<ByteRange>) -> Vec<ByteRange> {
        if ranges.is_empty() {
            return ranges;
        }

        // Sort by start position
        ranges.sort_by_key(|r| r.start);

        let mut merged = Vec::with_capacity(ranges.len());
        let mut current = ranges[0].clone();

        for range in ranges.into_iter().skip(1) {
            let current_end = current.end.unwrap_or(u64::MAX);

            // Check if ranges overlap or are adjacent (within 64KB is considered adjacent for BGZF)
            if range.start <= current_end + 65536 {
                // Extend current range
                current.end = match (current.end, range.end) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                };
            } else {
                merged.push(current);
                current = range;
            }
        }
        merged.push(current);

        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_bam_reader() {
        let path = std::path::Path::new("tests/data/mt.bam");
        if !path.exists() {
            return;
        }

        let file = std::fs::File::open(path).unwrap();
        // bam::io::Reader::new wraps the file in BGZF internally - don't double-wrap
        let mut reader = bam::io::Reader::new(file);
        let header = reader.read_header().unwrap();
        assert!(!header.reference_sequences().is_empty());
    }

    #[tokio::test]
    async fn test_async_bam_reader() {
        let path = std::path::Path::new("tests/data/mt.bam");
        if !path.exists() {
            return;
        }

        let file = File::open(path).await.unwrap();
        // bam::Reader::new wraps the file in BGZF internally - don't double-wrap
        let mut reader = bam::r#async::io::Reader::new(file);
        let header = reader.read_header().await.unwrap();
        assert!(!header.reference_sequences().is_empty());
    }
}
