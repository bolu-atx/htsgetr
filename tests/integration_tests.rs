//! Integration tests for htsgetr
//!
//! These tests require test data files in tests/data/

use axum_test::TestServer;
use htsgetr::{
    handlers::{AppState, create_router},
    storage::LocalStorage,
};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

fn test_data_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    eprintln!("Test data dir: {:?}", dir);
    eprintln!("Dir exists: {}", dir.exists());
    if dir.exists() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            eprintln!("  File: {:?}", entry.unwrap().path());
        }
    }
    dir
}

fn create_test_server() -> TestServer {
    let data_dir = test_data_dir();
    let base_url = "http://localhost:8080".to_string();

    // Debug: check if file exists
    let test_file = data_dir.join("mt.bam");
    eprintln!(
        "Looking for: {:?}, exists: {}",
        test_file,
        test_file.exists()
    );

    let storage = Arc::new(LocalStorage::new(data_dir, base_url.clone()));

    let state = AppState { storage, base_url };

    // Use centralized router definition
    let app = create_router(state);

    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn test_service_info() {
    let server = create_test_server();

    let response = server.get("/service-info").await;
    response.assert_status_ok();

    let body: Value = response.json();
    assert_eq!(body["type"]["artifact"], "htsget");
    assert_eq!(body["type"]["version"], "1.3.0");
}

#[tokio::test]
async fn test_storage_exists() {
    use htsgetr::storage::Storage;
    use htsgetr::types::Format;

    let data_dir = test_data_dir();
    let storage = LocalStorage::new(data_dir, "http://localhost:8080".to_string());

    let exists = storage.exists("mt", Format::Bam).await.unwrap();
    eprintln!("storage.exists('mt', BAM) = {}", exists);
    assert!(exists, "mt.bam should exist");
}

#[tokio::test]
async fn test_reads_endpoint_whole_file() {
    let server = create_test_server();

    let response = server.get("/reads/mt").await;
    eprintln!("Response status: {:?}", response.status_code());
    eprintln!("Response body: {:?}", response.text());
    response.assert_status_ok();

    let body: Value = response.json();
    assert_eq!(body["htsget"]["format"], "BAM");

    let urls = body["htsget"]["urls"].as_array().unwrap();
    assert!(!urls.is_empty());

    // When no region specified, should return whole file URL without byte ranges
    let url = urls[0]["url"].as_str().unwrap();
    assert!(url.contains("/data/reads/mt"));
}

#[tokio::test]
async fn test_reads_endpoint_with_region() {
    let server = create_test_server();

    // Query for chr1 (the test BAM file has chr1, chr2, etc. - not MT)
    let response = server
        .get("/reads/mt?referenceName=chr1&start=0&end=1000")
        .await;
    response.assert_status_ok();

    let body: Value = response.json();
    assert_eq!(body["htsget"]["format"], "BAM");

    let urls = body["htsget"]["urls"].as_array().unwrap();
    // With index available, should get header + data blocks
    assert!(!urls.is_empty());

    // First URL should be header block
    if urls.len() > 1 {
        assert_eq!(urls[0]["class"], "header");
    }
}

#[tokio::test]
async fn test_reads_endpoint_not_found() {
    let server = create_test_server();

    let response = server.get("/reads/nonexistent").await;
    response.assert_status_not_found();

    let body: Value = response.json();
    assert_eq!(body["htsget"]["error"], "NotFound");
}

#[tokio::test]
async fn test_reads_endpoint_header_only() {
    let server = create_test_server();

    let response = server.get("/reads/mt?class=header").await;
    response.assert_status_ok();

    let body: Value = response.json();
    let urls = body["htsget"]["urls"].as_array().unwrap();
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0]["class"], "header");

    // Header URL should have byte range
    let url = urls[0]["url"].as_str().unwrap();
    assert!(url.contains("start="));
}

#[tokio::test]
async fn test_data_endpoint_whole_file() {
    let server = create_test_server();

    let response = server.get("/data/reads/mt").await;
    response.assert_status_ok();

    // Should return binary BAM data
    let content_type = response.headers().get("content-type").unwrap();
    assert_eq!(content_type, "application/vnd.ga4gh.bam");

    // Should have content-length
    let content_length = response.headers().get("content-length").unwrap();
    assert!(content_length.to_str().unwrap().parse::<u64>().unwrap() > 0);
}

#[tokio::test]
async fn test_data_endpoint_partial_content() {
    let server = create_test_server();

    let response = server.get("/data/reads/mt?start=0&end=1000").await;

    // Should return 206 Partial Content
    response.assert_status(axum::http::StatusCode::PARTIAL_CONTENT);

    // Should have Content-Range header
    let content_range = response.headers().get("content-range").unwrap();
    let range_str = content_range.to_str().unwrap();
    assert!(range_str.starts_with("bytes 0-"));

    // Should have Accept-Ranges header
    let accept_ranges = response.headers().get("accept-ranges").unwrap();
    assert_eq!(accept_ranges, "bytes");
}

#[tokio::test]
async fn test_data_endpoint_not_found() {
    let server = create_test_server();

    let response = server.get("/data/reads/nonexistent").await;
    response.assert_status_not_found();
}

#[tokio::test]
async fn test_post_reads_with_regions() {
    let server = create_test_server();

    // Use chr1 (the test BAM file has chr1, chr2, etc. - not MT)
    let body = serde_json::json!({
        "format": "BAM",
        "regions": [
            {"referenceName": "chr1", "start": 0, "end": 1000}
        ]
    });

    let response = server.post("/reads/mt").json(&body).await;
    response.assert_status_ok();

    let resp_body: Value = response.json();
    assert_eq!(resp_body["htsget"]["format"], "BAM");
}

#[tokio::test]
async fn test_variants_endpoint_not_found() {
    let server = create_test_server();

    // We don't have a VCF.gz file, so this should return 404
    let response = server.get("/variants/sample").await;
    response.assert_status_not_found();
}

#[tokio::test]
async fn test_unsupported_format() {
    let server = create_test_server();

    // Try to get reads with VCF format (not a reads format)
    let response = server.get("/reads/mt?format=VCF").await;

    // Should return error for unsupported format
    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}
