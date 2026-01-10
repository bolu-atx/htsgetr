use crate::types::{Format, HtsgetCapabilities, Organization, ServiceInfo, ServiceType};
use axum::Json;

pub async fn service_info() -> Json<ServiceInfo> {
    Json(ServiceInfo {
        id: "org.example.htsgetr".to_string(),
        name: "htsgetr".to_string(),
        r#type: ServiceType {
            group: "org.ga4gh".to_string(),
            artifact: "htsget".to_string(),
            version: "1.3.0".to_string(),
        },
        description: Some("htsget protocol server implementation in Rust".to_string()),
        organization: Organization {
            name: "Example Organization".to_string(),
            url: "https://example.org".to_string(),
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        htsget: HtsgetCapabilities {
            datatype: "reads".to_string(),
            formats: vec![Format::Bam, Format::Cram, Format::Vcf, Format::Bcf],
            fields_parameter_effective: false,
            tags_parameters_effective: false,
        },
    })
}
