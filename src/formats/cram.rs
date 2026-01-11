use super::IndexedRanges;
use crate::storage::ByteRange;
use crate::types::Region;
use crate::{Error, Result};
use noodles::core::Position;
use noodles::core::region::Interval;
use noodles::cram;
use noodles::cram::crai;
use noodles::sam;
use std::path::Path;
use tokio::fs::File;

pub struct CramIndexReader;

impl CramIndexReader {
    /// Read CRAI index and compute byte ranges for given regions
    pub async fn query_ranges(
        cram_path: &Path,
        index_path: &Path,
        regions: &[Region],
    ) -> Result<IndexedRanges> {
        // Read the CRAI index
        let index = crai::r#async::read(index_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to read CRAI index: {}", e)))?;

        // Compute header byte range
        let header_range = Self::header_range(cram_path).await?;

        // If no regions specified, return empty data_ranges (caller should serve whole file)
        if regions.is_empty() {
            return Ok(IndexedRanges {
                header_range,
                data_ranges: vec![],
            });
        }

        // Read header to get reference sequence mapping
        let header = Self::read_header(cram_path).await?;
        let ref_seqs = header.reference_sequences();

        // Query index for each region
        let mut data_ranges: Vec<ByteRange> = Vec::new();

        for region in regions {
            // Map reference name to reference sequence ID
            let ref_id = ref_seqs
                .get_index_of(region.reference_name.as_bytes())
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

            let _interval = Interval::from(start..=end);

            // Find containers that overlap the region
            // CRAI records have: reference_sequence_id, alignment_start, alignment_span, offset, slice_offset, slice_length
            for record in index.iter() {
                // Check if this record matches our reference sequence
                if let Some(record_ref_id) = record.reference_sequence_id() {
                    if record_ref_id != ref_id {
                        continue;
                    }

                    // Check if the container overlaps our interval
                    let record_start = record.alignment_start();
                    let record_end = record_start.map(|s| {
                        s.checked_add(record.alignment_span())
                            .unwrap_or(Position::MAX)
                    });

                    // Check overlap
                    let overlaps = match (record_start, record_end) {
                        (Some(rs), Some(re)) => {
                            // Container spans rs..re
                            // Query spans start..end
                            // Overlap if: rs <= end && re >= start
                            rs <= end && re >= start
                        }
                        _ => {
                            // Unmapped or unknown - include if querying unmapped
                            false
                        }
                    };

                    if overlaps {
                        // Container offset is the compressed byte position
                        let container_offset = record.offset();
                        let slice_length = record.slice_length();

                        data_ranges.push(ByteRange {
                            start: container_offset,
                            end: Some(container_offset + slice_length),
                        });
                    }
                }
            }
        }

        // Merge overlapping/adjacent ranges
        data_ranges = Self::merge_ranges(data_ranges);

        Ok(IndexedRanges {
            header_range,
            data_ranges,
        })
    }

    /// Compute the header byte range for CRAM
    /// CRAM files have a file definition (26 bytes) followed by containers
    /// The first container is typically the header container
    pub async fn header_range(cram_path: &Path) -> Result<ByteRange> {
        let file = File::open(cram_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to open CRAM file: {}", e)))?;

        let mut reader = cram::r#async::io::Reader::new(file);

        // Read file definition (26 bytes)
        reader
            .read_file_definition()
            .await
            .map_err(|e| Error::Internal(format!("failed to read CRAM file definition: {}", e)))?;

        // Read header container to find where data starts
        reader
            .read_file_header()
            .await
            .map_err(|e| Error::Internal(format!("failed to read CRAM header: {}", e)))?;

        // The position after header container
        // This is approximate - we'd need to track the actual byte position
        // For now, return a reasonable estimate based on typical header sizes
        Ok(ByteRange {
            start: 0,
            end: Some(65536), // Conservative estimate
        })
    }

    /// Read the CRAM header (SAM header)
    pub async fn read_header(cram_path: &Path) -> Result<sam::Header> {
        let file = File::open(cram_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to open CRAM file: {}", e)))?;

        let mut reader = cram::r#async::io::Reader::new(file);

        // Read file definition
        reader
            .read_file_definition()
            .await
            .map_err(|e| Error::Internal(format!("failed to read CRAM file definition: {}", e)))?;

        // Read SAM header as string and parse it
        let header_str = reader
            .read_file_header()
            .await
            .map_err(|e| Error::Internal(format!("failed to read CRAM header: {}", e)))?;

        // Parse the header string into a sam::Header
        header_str
            .parse()
            .map_err(|e| Error::Internal(format!("failed to parse CRAM header: {}", e)))
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

            // Check if ranges overlap or are adjacent
            if range.start <= current_end + 1 {
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
