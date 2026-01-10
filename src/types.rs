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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_default() {
        assert_eq!(Format::default(), Format::Bam);
    }

    #[test]
    fn test_format_content_type() {
        assert_eq!(Format::Bam.content_type(), "application/vnd.ga4gh.bam");
        assert_eq!(Format::Cram.content_type(), "application/vnd.ga4gh.cram");
        assert_eq!(Format::Vcf.content_type(), "application/vnd.ga4gh.vcf");
        assert_eq!(Format::Bcf.content_type(), "application/vnd.ga4gh.bcf");
        assert_eq!(Format::Fasta.content_type(), "text/x-fasta");
        assert_eq!(Format::Fastq.content_type(), "text/x-fastq");
    }

    #[test]
    fn test_format_is_reads() {
        assert!(Format::Bam.is_reads());
        assert!(Format::Cram.is_reads());
        assert!(!Format::Vcf.is_reads());
        assert!(!Format::Bcf.is_reads());
        assert!(!Format::Fasta.is_reads());
        assert!(!Format::Fastq.is_reads());
    }

    #[test]
    fn test_format_is_variants() {
        assert!(!Format::Bam.is_variants());
        assert!(!Format::Cram.is_variants());
        assert!(Format::Vcf.is_variants());
        assert!(Format::Bcf.is_variants());
        assert!(!Format::Fasta.is_variants());
        assert!(!Format::Fastq.is_variants());
    }

    #[test]
    fn test_format_is_sequences() {
        assert!(!Format::Bam.is_sequences());
        assert!(!Format::Cram.is_sequences());
        assert!(!Format::Vcf.is_sequences());
        assert!(!Format::Bcf.is_sequences());
        assert!(Format::Fasta.is_sequences());
        assert!(Format::Fastq.is_sequences());
    }

    #[test]
    fn test_data_class_default() {
        assert_eq!(DataClass::default(), DataClass::Body);
    }

    #[test]
    fn test_format_serialization() {
        assert_eq!(serde_json::to_string(&Format::Bam).unwrap(), "\"BAM\"");
        assert_eq!(serde_json::to_string(&Format::Vcf).unwrap(), "\"VCF\"");
    }

    #[test]
    fn test_format_deserialization() {
        assert_eq!(
            serde_json::from_str::<Format>("\"BAM\"").unwrap(),
            Format::Bam
        );
        assert_eq!(
            serde_json::from_str::<Format>("\"VCF\"").unwrap(),
            Format::Vcf
        );
    }

    #[test]
    fn test_data_class_serialization() {
        assert_eq!(serde_json::to_string(&DataClass::Body).unwrap(), "\"body\"");
        assert_eq!(
            serde_json::to_string(&DataClass::Header).unwrap(),
            "\"header\""
        );
    }

    #[test]
    fn test_htsget_response_serialization() {
        let response = HtsgetResponse {
            htsget: HtsgetResponseBody {
                format: Format::Bam,
                urls: vec![UrlEntry {
                    url: "http://example.com/data".to_string(),
                    headers: None,
                    class: Some(DataClass::Body),
                }],
                md5: None,
            },
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"format\":\"BAM\""));
        assert!(json.contains("\"url\":\"http://example.com/data\""));
        assert!(json.contains("\"class\":\"body\""));
        // md5 should be omitted when None
        assert!(!json.contains("\"md5\""));
    }

    #[test]
    fn test_reads_query_deserialization() {
        let json = r#"{"format":"BAM","referenceName":"chr1","start":100,"end":200}"#;
        let query: ReadsQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.format, Some(Format::Bam));
        assert_eq!(query.reference_name, Some("chr1".to_string()));
        assert_eq!(query.start, Some(100));
        assert_eq!(query.end, Some(200));
    }

    #[test]
    fn test_region_deserialization() {
        let json = r#"{"referenceName":"chr1","start":0,"end":1000}"#;
        let region: Region = serde_json::from_str(json).unwrap();
        assert_eq!(region.reference_name, "chr1");
        assert_eq!(region.start, Some(0));
        assert_eq!(region.end, Some(1000));
    }
}
