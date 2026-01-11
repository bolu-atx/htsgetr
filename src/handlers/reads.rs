use super::AppState;
use crate::{
    Error, Result,
    formats::{BamIndexReader, CramIndexReader},
    types::{
        DataClass, Format, HtsgetResponse, HtsgetResponseBody, ReadsPostBody, ReadsQuery, Region,
        UrlEntry,
    },
};
use axum::{
    Json,
    extract::{Path, Query, State},
};

pub async fn get_reads(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ReadsQuery>,
) -> Result<Json<HtsgetResponse>> {
    tracing::debug!("get_reads: id={}, query={:?}", id, query);

    let format = query.format.unwrap_or(Format::Bam);
    tracing::debug!("get_reads: format={:?}", format);

    if !format.is_reads() {
        return Err(Error::UnsupportedFormat(format!(
            "{:?} is not a reads format",
            format
        )));
    }

    // Check file exists
    let file_path = state.storage.file_path(&id, format);
    tracing::debug!(
        "get_reads: file_path={:?}, exists={}",
        file_path,
        file_path.exists()
    );

    if !state.storage.exists(&id, format).await? {
        return Err(Error::NotFound(id));
    }

    let class = query.class.unwrap_or_default();
    let regions = match (&query.reference_name, query.start, query.end) {
        (Some(ref_name), start, end) => vec![Region {
            reference_name: ref_name.clone(),
            start,
            end,
        }],
        _ => vec![],
    };

    build_reads_response(&state, &id, format, class, &regions).await
}

pub async fn post_reads(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReadsPostBody>,
) -> Result<Json<HtsgetResponse>> {
    let format = body.format.unwrap_or(Format::Bam);

    if !format.is_reads() {
        return Err(Error::UnsupportedFormat(format!(
            "{:?} is not a reads format",
            format
        )));
    }

    if !state.storage.exists(&id, format).await? {
        return Err(Error::NotFound(id));
    }

    let class = body.class.unwrap_or_default();
    let regions = body.regions.unwrap_or_default();

    build_reads_response(&state, &id, format, class, &regions).await
}

async fn build_reads_response(
    state: &AppState,
    id: &str,
    format: Format,
    class: DataClass,
    regions: &[Region],
) -> Result<Json<HtsgetResponse>> {
    let mut urls = Vec::new();
    let file_path = state.storage.file_path(id, format);

    match class {
        DataClass::Header => {
            // Return only the header block - dispatch based on format
            let header_range = match format {
                Format::Bam => BamIndexReader::header_range(&file_path).await?,
                Format::Cram => CramIndexReader::header_range(&file_path).await?,
                _ => return Err(Error::UnsupportedFormat(format!("{:?}", format))),
            };
            urls.push(UrlEntry {
                url: state.storage.data_url(id, format, Some(header_range)),
                headers: None,
                class: Some(DataClass::Header),
            });
        }
        DataClass::Body => {
            if regions.is_empty() {
                // No regions - return entire file
                urls.push(UrlEntry {
                    url: state.storage.data_url(id, format, None),
                    headers: None,
                    class: None,
                });
            } else {
                // Check if index is available
                let index_path = state.storage.index_path(id, format).await?;

                if let Some(idx_path) = index_path {
                    // Query index for byte ranges - dispatch based on format
                    let indexed = match format {
                        Format::Bam => {
                            let header = BamIndexReader::read_header(&file_path).await?;
                            BamIndexReader::query_ranges(&file_path, &idx_path, regions, &header)
                                .await?
                        }
                        Format::Cram => {
                            CramIndexReader::query_ranges(&file_path, &idx_path, regions).await?
                        }
                        _ => return Err(Error::UnsupportedFormat(format!("{:?}", format))),
                    };

                    // Add header block first
                    urls.push(UrlEntry {
                        url: state
                            .storage
                            .data_url(id, format, Some(indexed.header_range)),
                        headers: None,
                        class: Some(DataClass::Header),
                    });

                    // Add data blocks
                    if indexed.data_ranges.is_empty() {
                        // Index query returned no specific ranges - return whole file body
                        // This shouldn't happen if index was properly queried
                        urls.push(UrlEntry {
                            url: state.storage.data_url(id, format, None),
                            headers: None,
                            class: Some(DataClass::Body),
                        });
                    } else {
                        for range in indexed.data_ranges {
                            urls.push(UrlEntry {
                                url: state.storage.data_url(id, format, Some(range)),
                                headers: None,
                                class: Some(DataClass::Body),
                            });
                        }
                    }
                } else {
                    // No index available - return whole file
                    urls.push(UrlEntry {
                        url: state.storage.data_url(id, format, None),
                        headers: None,
                        class: None,
                    });
                }
            }
        }
    }

    Ok(Json(HtsgetResponse {
        htsget: HtsgetResponseBody {
            format,
            urls,
            md5: None,
        },
    }))
}
