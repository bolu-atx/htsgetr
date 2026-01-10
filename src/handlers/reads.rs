use axum::{
    Json,
    extract::{Path, Query, State},
};
use crate::{
    Error, Result,
    types::{
        DataClass, Format, HtsgetResponse, HtsgetResponseBody,
        ReadsPostBody, ReadsQuery, Region, UrlEntry,
    },
};
use super::AppState;

pub async fn get_reads(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ReadsQuery>,
) -> Result<Json<HtsgetResponse>> {
    let format = query.format.unwrap_or(Format::Bam);

    if !format.is_reads() {
        return Err(Error::UnsupportedFormat(format!("{:?} is not a reads format", format)));
    }

    // Check file exists
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
        return Err(Error::UnsupportedFormat(format!("{:?} is not a reads format", format)));
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

    match class {
        DataClass::Header => {
            // Return only the header block
            urls.push(UrlEntry {
                url: state.storage.data_url(id, format, None),
                headers: None,
                class: Some(DataClass::Header),
            });
        }
        DataClass::Body => {
            if regions.is_empty() {
                // Return entire file
                urls.push(UrlEntry {
                    url: state.storage.data_url(id, format, None),
                    headers: None,
                    class: None,
                });
            } else {
                // TODO: Use index to compute byte ranges for each region
                // For now, return the whole file (inefficient but correct)
                urls.push(UrlEntry {
                    url: state.storage.data_url(id, format, None),
                    headers: None,
                    class: None,
                });
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
