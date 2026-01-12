//! HTTP/HTTPS storage backend for genomic data files.
//!
//! This module provides an HTTP-based implementation of the [`Storage`] trait,
//! enabling htsgetr to serve genomic data from remote HTTP/HTTPS URLs.
//!
//! # Features
//!
//! - Direct URL access for data (clients fetch from remote server)
//! - Local caching of index files for efficient repeated queries
//! - Support for HTTP Range requests

use super::{ByteRange, FileInfo, Storage};
use crate::{Error, Result, types::Format};
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::Client;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// HTTP/HTTPS storage backend for genomic data files.
pub struct HttpStorage {
    client: Client,
    base_url: String,
    index_base_url: Option<String>,
    cache_dir: PathBuf,
}

impl HttpStorage {
    /// Create a new HttpStorage instance.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL for data files (e.g., "https://example.com/data/")
    /// * `index_base_url` - Optional separate base URL for index files
    /// * `cache_dir` - Local directory for caching index files
    pub async fn new(
        base_url: String,
        index_base_url: Option<String>,
        cache_dir: PathBuf,
    ) -> Result<Self> {
        let client = Client::builder()
            .build()
            .map_err(|e| Error::Internal(format!("failed to create HTTP client: {}", e)))?;

        // Ensure cache directory exists
        fs::create_dir_all(&cache_dir)
            .await
            .map_err(|e| Error::Internal(format!("failed to create cache dir: {}", e)))?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            index_base_url: index_base_url.map(|u| u.trim_end_matches('/').to_string()),
            cache_dir,
        })
    }

    /// Construct the URL for a data file.
    fn file_url(&self, id: &str, format: Format) -> String {
        let ext = Self::file_extension(format);
        format!("{}/{}.{}", self.base_url, id, ext)
    }

    /// Construct the URL for an index file.
    fn index_url(&self, id: &str, format: Format, appended: bool) -> Option<String> {
        let idx_ext = Self::index_extension(format)?;
        let data_ext = Self::file_extension(format);
        let base = self.index_base_url.as_ref().unwrap_or(&self.base_url);

        Some(if appended {
            // e.g., sample.bam.bai
            format!("{}/{}.{}.{}", base, id, data_ext, idx_ext)
        } else {
            // e.g., sample.bai
            format!("{}/{}.{}", base, id, idx_ext)
        })
    }

    fn file_extension(format: Format) -> &'static str {
        match format {
            Format::Bam => "bam",
            Format::Cram => "cram",
            Format::Vcf => "vcf.gz",
            Format::Bcf => "bcf",
            Format::Fasta => "fa",
            Format::Fastq => "fq.gz",
        }
    }

    fn index_extension(format: Format) -> Option<&'static str> {
        match format {
            Format::Bam => Some("bai"),
            Format::Cram => Some("crai"),
            Format::Vcf => Some("tbi"),
            Format::Bcf => Some("csi"),
            Format::Fasta => Some("fai"),
            Format::Fastq => None,
        }
    }

    /// Get the local cache path for an index file.
    fn index_cache_path(&self, id: &str, format: Format, appended: bool) -> PathBuf {
        let ext = Self::file_extension(format);
        let idx_ext = Self::index_extension(format).unwrap_or("idx");

        if appended {
            self.cache_dir.join(format!("{}.{}.{}", id, ext, idx_ext))
        } else {
            self.cache_dir.join(format!("{}.{}", id, idx_ext))
        }
    }

    /// Get the local cache path for a data file (used for header reading).
    fn data_cache_path(&self, id: &str, format: Format) -> PathBuf {
        let ext = Self::file_extension(format);
        self.cache_dir.join(format!("{}.{}", id, ext))
    }

    /// Check if a URL exists via HEAD request.
    async fn url_exists(&self, url: &str) -> bool {
        self.client
            .head(url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Get the content length of a URL via HEAD request.
    async fn get_content_length(&self, url: &str) -> Result<u64> {
        let response = self
            .client
            .head(url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("HTTP HEAD request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::NotFound(url.to_string()));
        }

        response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| Error::Internal("missing Content-Length header".to_string()))
    }

    /// Download a URL to a local file.
    async fn download_to_cache(&self, url: &str, cache_path: &PathBuf) -> Result<()> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("HTTP GET request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::NotFound(url.to_string()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Internal(format!("failed to read HTTP response: {}", e)))?;

        let mut file = fs::File::create(cache_path)
            .await
            .map_err(|e| Error::Internal(format!("failed to create cache file: {}", e)))?;

        file.write_all(&bytes)
            .await
            .map_err(|e| Error::Internal(format!("failed to write cache file: {}", e)))?;

        Ok(())
    }

    /// Download a byte range from a URL.
    async fn download_range(&self, url: &str, range: Option<&ByteRange>) -> Result<Bytes> {
        let mut request = self.client.get(url);

        if let Some(r) = range {
            let range_header = match r.end {
                Some(end) => format!("bytes={}-{}", r.start, end),
                None => format!("bytes={}-", r.start),
            };
            request = request.header(reqwest::header::RANGE, range_header);
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Internal(format!("HTTP GET request failed: {}", e)))?;

        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(Error::NotFound(url.to_string()));
        }

        response
            .bytes()
            .await
            .map_err(|e| Error::Internal(format!("failed to read HTTP response: {}", e)))
    }
}

#[async_trait]
impl Storage for HttpStorage {
    async fn exists(&self, id: &str, format: Format) -> Result<bool> {
        let url = self.file_url(id, format);
        Ok(self.url_exists(&url).await)
    }

    async fn file_info(&self, id: &str, format: Format) -> Result<FileInfo> {
        let url = self.file_url(id, format);
        let size = self.get_content_length(&url).await?;

        // Check if index exists (try both naming conventions)
        let has_index = if let Some(appended_url) = self.index_url(id, format, true) {
            if self.url_exists(&appended_url).await {
                true
            } else if let Some(replaced_url) = self.index_url(id, format, false) {
                self.url_exists(&replaced_url).await
            } else {
                false
            }
        } else {
            false
        };

        Ok(FileInfo {
            id: id.to_string(),
            format,
            size,
            has_index,
        })
    }

    fn data_url(&self, id: &str, format: Format, _range: Option<ByteRange>) -> String {
        // Return the direct HTTP URL
        // htsget clients will use Range headers when fetching
        self.file_url(id, format)
    }

    async fn read_bytes(
        &self,
        id: &str,
        format: Format,
        range: Option<ByteRange>,
    ) -> Result<Bytes> {
        let url = self.file_url(id, format);
        self.download_range(&url, range.as_ref()).await
    }

    async fn index_path(&self, id: &str, format: Format) -> Result<Option<PathBuf>> {
        // Try appended index first (e.g., sample.bam.bai)
        if let Some(url) = self.index_url(id, format, true) {
            let cache_path = self.index_cache_path(id, format, true);

            // Check cache first
            if cache_path.exists() {
                return Ok(Some(cache_path));
            }

            // Check if exists remotely and download
            if self.url_exists(&url).await {
                self.download_to_cache(&url, &cache_path).await?;
                return Ok(Some(cache_path));
            }
        }

        // Try replaced extension (e.g., sample.bai)
        if let Some(url) = self.index_url(id, format, false) {
            let cache_path = self.index_cache_path(id, format, false);

            if cache_path.exists() {
                return Ok(Some(cache_path));
            }

            if self.url_exists(&url).await {
                self.download_to_cache(&url, &cache_path).await?;
                return Ok(Some(cache_path));
            }
        }

        Ok(None)
    }

    fn file_path(&self, id: &str, format: Format) -> PathBuf {
        // Return path in cache directory
        // Note: The file may not exist locally yet - handlers should ensure
        // the header is downloaded when needed
        self.data_cache_path(id, format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_url() {
        let base_url = "https://example.com/data";
        let url = format!("{}/{}.{}", base_url, "sample1", "bam");
        assert_eq!(url, "https://example.com/data/sample1.bam");
    }

    #[test]
    fn test_file_extensions() {
        assert_eq!(HttpStorage::file_extension(Format::Bam), "bam");
        assert_eq!(HttpStorage::file_extension(Format::Cram), "cram");
        assert_eq!(HttpStorage::file_extension(Format::Vcf), "vcf.gz");
        assert_eq!(HttpStorage::file_extension(Format::Bcf), "bcf");
        assert_eq!(HttpStorage::file_extension(Format::Fasta), "fa");
        assert_eq!(HttpStorage::file_extension(Format::Fastq), "fq.gz");
    }

    #[test]
    fn test_index_extensions() {
        assert_eq!(HttpStorage::index_extension(Format::Bam), Some("bai"));
        assert_eq!(HttpStorage::index_extension(Format::Cram), Some("crai"));
        assert_eq!(HttpStorage::index_extension(Format::Vcf), Some("tbi"));
        assert_eq!(HttpStorage::index_extension(Format::Bcf), Some("csi"));
        assert_eq!(HttpStorage::index_extension(Format::Fasta), Some("fai"));
        assert_eq!(HttpStorage::index_extension(Format::Fastq), None);
    }

    #[test]
    fn test_index_cache_path() {
        let cache_dir = PathBuf::from("/tmp/cache");

        // Test appended style
        let path = cache_dir.join(format!("{}.{}.{}", "sample1", "bam", "bai"));
        assert_eq!(path, PathBuf::from("/tmp/cache/sample1.bam.bai"));

        // Test replaced style
        let path = cache_dir.join(format!("{}.{}", "sample1", "bai"));
        assert_eq!(path, PathBuf::from("/tmp/cache/sample1.bai"));
    }
}
