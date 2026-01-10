use serde::{Deserialize, Serialize};

/// htsget response format per spec 1.3
#[derive(Debug, Serialize)]
pub struct HtsgetResponse {
    pub htsget: HtsgetResponseBody,
}

#[derive(Debug, Serialize)]
pub struct HtsgetResponseBody {
    pub format: Format,
    pub urls: Vec<UrlEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UrlEntry {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<DataClass>,
}

/// Data formats supported by htsget
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum Format {
    #[default]
    Bam,
    Cram,
    Vcf,
    Bcf,
    // Extensions beyond spec
    Fasta,
    Fastq,
}

impl Format {
    pub fn content_type(&self) -> &'static str {
        match self {
            Format::Bam => "application/vnd.ga4gh.bam",
            Format::Cram => "application/vnd.ga4gh.cram",
            Format::Vcf => "application/vnd.ga4gh.vcf",
            Format::Bcf => "application/vnd.ga4gh.bcf",
            Format::Fasta => "text/x-fasta",
            Format::Fastq => "text/x-fastq",
        }
    }

    pub fn is_reads(&self) -> bool {
        matches!(self, Format::Bam | Format::Cram)
    }

    pub fn is_variants(&self) -> bool {
        matches!(self, Format::Vcf | Format::Bcf)
    }

    pub fn is_sequences(&self) -> bool {
        matches!(self, Format::Fasta | Format::Fastq)
    }
}

/// Data class - header only or full data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DataClass {
    #[default]
    Body,
    Header,
}

/// Query parameters for GET requests
#[derive(Debug, Deserialize, Default)]
pub struct ReadsQuery {
    pub format: Option<Format>,
    pub class: Option<DataClass>,
    #[serde(rename = "referenceName")]
    pub reference_name: Option<String>,
    pub start: Option<u64>,
    pub end: Option<u64>,
    pub fields: Option<String>,
    pub tags: Option<String>,
    pub notags: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct VariantsQuery {
    pub format: Option<Format>,
    pub class: Option<DataClass>,
    #[serde(rename = "referenceName")]
    pub reference_name: Option<String>,
    pub start: Option<u64>,
    pub end: Option<u64>,
}

/// POST request body for multiple regions
#[derive(Debug, Deserialize)]
pub struct ReadsPostBody {
    pub format: Option<Format>,
    pub class: Option<DataClass>,
    pub regions: Option<Vec<Region>>,
    pub fields: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub notags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct VariantsPostBody {
    pub format: Option<Format>,
    pub class: Option<DataClass>,
    pub regions: Option<Vec<Region>>,
}

#[derive(Debug, Deserialize)]
pub struct Region {
    #[serde(rename = "referenceName")]
    pub reference_name: String,
    pub start: Option<u64>,
    pub end: Option<u64>,
}

/// Service info response (GA4GH service-info spec)
#[derive(Debug, Serialize)]
pub struct ServiceInfo {
    pub id: String,
    pub name: String,
    pub r#type: ServiceType,
    pub description: Option<String>,
    pub organization: Organization,
    pub version: String,
    pub htsget: HtsgetCapabilities,
}

#[derive(Debug, Serialize)]
pub struct ServiceType {
    pub group: String,
    pub artifact: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct Organization {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct HtsgetCapabilities {
    pub datatype: String,
    pub formats: Vec<Format>,
    #[serde(rename = "fieldsParameterEffective")]
    pub fields_parameter_effective: bool,
    #[serde(rename = "tagsParametersEffective")]
    pub tags_parameters_effective: bool,
}
