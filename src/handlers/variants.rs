use super::AppState;
use crate::{
    Error, Result,
    formats::VcfIndexReader,
    types::{
        DataClass, Format, HtsgetResponse, HtsgetResponseBody, Region, UrlEntry, VariantsPostBody,
        VariantsQuery,
    },
};
use axum::{
    Json,
    extract::{Path, Query, State},
};

pub async fn get_variants(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<VariantsQuery>,
) -> Result<Json<HtsgetResponse>> {
    let format = query.format.unwrap_or(Format::Vcf);

    if !format.is_variants() {
        return Err(Error::UnsupportedFormat(format!(
            "{:?} is not a variants format",
            format
        )));
    }

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

    build_variants_response(&state, &id, format, class, &regions).await
}

pub async fn post_variants(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<VariantsPostBody>,
) -> Result<Json<HtsgetResponse>> {
    let format = body.format.unwrap_or(Format::Vcf);

    if !format.is_variants() {
        return Err(Error::UnsupportedFormat(format!(
            "{:?} is not a variants format",
            format
        )));
    }

    if !state.storage.exists(&id, format).await? {
        return Err(Error::NotFound(id));
    }

    let class = body.class.unwrap_or_default();
    let regions = body.regions.unwrap_or_default();

    build_variants_response(&state, &id, format, class, &regions).await
}

async fn build_variants_response(
    state: &AppState,
    id: &str,
    format: Format,
    class: DataClass,
    regions: &[Region],
) -> Result<Json<HtsgetResponse>> {
    let mut urls = Vec::new();
    let vcf_path = state.storage.file_path(id, format);

    match class {
        DataClass::Header => {
            // Return only the header block
            let header_range = VcfIndexReader::header_range(&vcf_path).await?;
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
                    // Query tabix index for byte ranges
                    let indexed =
                        VcfIndexReader::query_ranges(&vcf_path, &idx_path, regions).await?;

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
