use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
};
use serde::Deserialize;
use crate::{Error, Result, types::Format};
use crate::storage::ByteRange;
use super::AppState;

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

    let bytes = state.storage.read_bytes(&id, format, range).await?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, format.content_type())
        .header(header::CONTENT_LENGTH, bytes.len())
        .body(Body::from(bytes))
        .unwrap();

    Ok(response)
}

fn parse_format(s: &str) -> Result<Format> {
    match s {
        "reads" => Ok(Format::Bam),
        "variants" => Ok(Format::Vcf),
        "sequences" => Ok(Format::Fasta),
        _ => Err(Error::InvalidInput(format!("unknown format path: {}", s))),
    }
}
