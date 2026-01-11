use super::IndexedRanges;
use crate::storage::ByteRange;
use crate::types::Region;
use crate::{Error, Result};
use noodles::fasta::fai;
use std::path::Path;

pub struct FastaIndexReader;

impl FastaIndexReader {
    /// Read FAI index and compute byte ranges for given regions
    pub async fn query_ranges(
        _fasta_path: &Path,
        index_path: &Path,
        regions: &[Region],
    ) -> Result<IndexedRanges> {
        // Read the FAI index (synchronous, then wrap in async context)
        let index = tokio::task::spawn_blocking({
            let path = index_path.to_path_buf();
            move || fai::read(&path)
        })
        .await
        .map_err(|e| Error::Internal(format!("failed to read FAI index: {}", e)))?
        .map_err(|e| Error::Internal(format!("failed to read FAI index: {}", e)))?;

        // FASTA doesn't have a separate header - the index tells us where each sequence is
        // For htsget, we return byte ranges for the requested sequences
        let header_range = ByteRange {
            start: 0,
            end: Some(0), // No header for FASTA
        };

        // If no regions specified, return empty data_ranges (caller should serve whole file)
        if regions.is_empty() {
            return Ok(IndexedRanges {
                header_range,
                data_ranges: vec![],
            });
        }

        // Query index for each region
        let mut data_ranges: Vec<ByteRange> = Vec::new();

        for region in regions {
            // Find the sequence in the index
            // FAI Index wraps Vec<Record>, access via as_ref()
            let record = index
                .as_ref()
                .iter()
                .find(|r| r.name() == region.reference_name.as_bytes())
                .ok_or_else(|| {
                    Error::NotFound(format!("sequence not found: {}", region.reference_name))
                })?;

            // FAI record contains:
            // - name: sequence name
            // - length: sequence length in bases
            // - offset: byte offset of first base
            // - line_bases: bases per line
            // - line_width: bytes per line (including newline)

            let seq_length = record.length() as u64;
            let offset = record.offset();
            let line_bases = record.line_bases() as u64;
            let line_width = record.line_width() as u64;

            // Calculate byte range for the requested region
            let start_base = region.start.unwrap_or(0);
            let end_base = region.end.unwrap_or(seq_length).min(seq_length);

            // Convert base coordinates to byte offsets
            // Each line has line_bases bases and line_width bytes
            let start_line = start_base / line_bases;
            let end_line = (end_base - 1) / line_bases;

            let byte_start = offset + start_line * line_width + (start_base % line_bases);
            let byte_end = offset + end_line * line_width + ((end_base - 1) % line_bases) + 1;

            data_ranges.push(ByteRange {
                start: byte_start,
                end: Some(byte_end),
            });
        }

        // Merge overlapping/adjacent ranges
        data_ranges = Self::merge_ranges(data_ranges);

        Ok(IndexedRanges {
            header_range,
            data_ranges,
        })
    }

    /// Get header byte range for FASTA (there is no header)
    pub async fn header_range(_fasta_path: &Path) -> Result<ByteRange> {
        // FASTA files don't have a header in the htsget sense
        // Return an empty range
        Ok(ByteRange {
            start: 0,
            end: Some(0),
        })
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
