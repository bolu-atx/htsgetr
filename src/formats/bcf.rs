use super::IndexedRanges;
use crate::storage::ByteRange;
use crate::types::Region;
use crate::{Error, Result};
use noodles::bcf;
use noodles::bgzf;
use noodles::core::Position;
use noodles::core::region::Interval;
use noodles::csi;
use noodles::csi::binning_index::BinningIndex;
use noodles::csi::binning_index::index::reference_sequence::bin::Chunk;
use std::path::Path;
use tokio::fs::File;

pub struct BcfIndexReader;

impl BcfIndexReader {
    /// Read CSI index and compute byte ranges for given regions
    pub async fn query_ranges(
        bcf_path: &Path,
        index_path: &Path,
        regions: &[Region],
    ) -> Result<IndexedRanges> {
        // Read the CSI index
        let index = csi::r#async::read(index_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to read CSI index: {}", e)))?;

        // Compute header byte range
        let header_range = Self::header_range(bcf_path).await?;

        // If no regions specified, return empty data_ranges (caller should serve whole file)
        if regions.is_empty() {
            return Ok(IndexedRanges {
                header_range,
                data_ranges: vec![],
            });
        }

        // Read header to get reference sequence mapping
        let header = Self::read_header(bcf_path).await?;

        // Query index for each region
        let mut chunks: Vec<Chunk> = Vec::new();

        for region in regions {
            // Map reference name to reference sequence ID using header contigs
            let ref_id = header
                .contigs()
                .get_index_of(&region.reference_name)
                .ok_or_else(|| {
                    Error::NotFound(format!(
                        "reference sequence not found: {}",
                        region.reference_name
                    ))
                })?;

            // Build interval from region coordinates
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

        // Merge overlapping/adjacent ranges
        data_ranges = Self::merge_ranges(data_ranges);

        Ok(IndexedRanges {
            header_range,
            data_ranges,
        })
    }

    /// Compute the header byte range by reading the BCF file
    pub async fn header_range(bcf_path: &Path) -> Result<ByteRange> {
        let file = File::open(bcf_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to open BCF file: {}", e)))?;

        let mut reader = bcf::r#async::io::Reader::new(bgzf::r#async::Reader::new(file));

        // Read header to advance position past it
        reader
            .read_header()
            .await
            .map_err(|e| Error::Internal(format!("failed to read BCF header: {}", e)))?;

        // Get the virtual position after the header
        let header_end = reader.get_ref().virtual_position();

        Ok(ByteRange {
            start: 0,
            end: Some(header_end.compressed()),
        })
    }

    /// Read the BCF header
    pub async fn read_header(bcf_path: &Path) -> Result<noodles::vcf::Header> {
        let file = File::open(bcf_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to open BCF file: {}", e)))?;

        let mut reader = bcf::r#async::io::Reader::new(bgzf::r#async::Reader::new(file));

        reader
            .read_header()
            .await
            .map_err(|e| Error::Internal(format!("failed to read BCF header: {}", e)))
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

            // Check if ranges overlap or are adjacent (within 64KB for BGZF)
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
