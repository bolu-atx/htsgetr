use super::AppState;
use crate::{
    Error, Result,
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

    match class {
        DataClass::Header => {
            urls.push(UrlEntry {
                url: state.storage.data_url(id, format, None),
                headers: None,
                class: Some(DataClass::Header),
            });
        }
        DataClass::Body => {
            if regions.is_empty() {
                urls.push(UrlEntry {
                    url: state.storage.data_url(id, format, None),
                    headers: None,
                    class: None,
                });
            } else {
                // TODO: Use tabix/csi index for byte ranges
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
