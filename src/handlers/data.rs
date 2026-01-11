use super::AppState;
use crate::storage::ByteRange;
use crate::{Error, Result, types::Format};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DataQuery {
    pub start: Option<u64>,
    pub end: Option<u64>,
}

/// Serve raw data blocks - this is what the ticket URLs point to
pub async fn get_data(
    State(state): State<AppState>,
    Path((format_str, id)): Path<(String, String)>,
    Query(query): Query<DataQuery>,
) -> Result<Response> {
    let format = parse_format(&format_str)?;

    if !state.storage.exists(&id, format).await? {
        return Err(Error::NotFound(id));
    }

    let range = match (query.start, query.end) {
        (Some(start), end) => Some(ByteRange { start, end }),
        _ => None,
    };

    let bytes = state.storage.read_bytes(&id, format, range.clone()).await?;

    // Determine response status and headers based on whether range was requested
    let (status, content_range) = if let Some(ref r) = range {
        // Get total file size for Content-Range header
        let file_info = state.storage.file_info(&id, format).await?;
        let total_size = file_info.size;

        // Calculate actual byte range returned
        let start = r.start;
        let actual_end = start + bytes.len() as u64;

        // Content-Range: bytes start-end/total
        let content_range = format!("bytes {}-{}/{}", start, actual_end - 1, total_size);

        (StatusCode::PARTIAL_CONTENT, Some(content_range))
    } else {
        (StatusCode::OK, None)
    };

    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, format.content_type())
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::ACCEPT_RANGES, "bytes");

    if let Some(cr) = content_range {
        builder = builder.header(header::CONTENT_RANGE, cr);
    }

    Ok(builder.body(Body::from(bytes)).unwrap())
}

fn parse_format(s: &str) -> Result<Format> {
    match s {
        "reads" => Ok(Format::Bam),
        "variants" => Ok(Format::Vcf),
        "sequences" => Ok(Format::Fasta),
        _ => Err(Error::InvalidInput(format!("unknown format path: {}", s))),
    }
}
