use super::AppState;
use crate::{
    Error, Result,
    types::{Format, HtsgetResponse, HtsgetResponseBody, UrlEntry},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct SequencesQuery {
    pub format: Option<Format>,
    #[serde(rename = "referenceName")]
    pub reference_name: Option<String>,
    pub start: Option<u64>,
    pub end: Option<u64>,
}

/// Extension endpoint for FASTA/FASTQ access (not part of htsget spec)
pub async fn get_sequences(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<SequencesQuery>,
) -> Result<Json<HtsgetResponse>> {
    let format = query.format.unwrap_or(Format::Fasta);

    if !format.is_sequences() {
        return Err(Error::UnsupportedFormat(format!(
            "{:?} is not a sequence format",
            format
        )));
    }

    if !state.storage.exists(&id, format).await? {
        return Err(Error::NotFound(id));
    }

    // For FASTA with .fai index, we could support region queries
    // For now, return the whole file
    let urls = vec![UrlEntry {
        url: state.storage.data_url(&id, format, None),
        headers: None,
        class: None,
    }];

    Ok(Json(HtsgetResponse {
        htsget: HtsgetResponseBody {
            format,
            urls,
            md5: None,
        },
    }))
}
