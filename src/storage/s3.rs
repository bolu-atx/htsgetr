//! S3 storage backend for genomic data files.
//!
//! This module provides an S3-based implementation of the [`Storage`] trait,
//! enabling htsgetr to serve genomic data directly from S3 buckets.
//!
//! # Features
//!
//! - Presigned URLs for direct client-to-S3 data access
//! - Local caching of index files for efficient repeated queries
//! - Support for custom S3 endpoints (MinIO, LocalStack, etc.)

use super::{ByteRange, FileInfo, Storage};
use crate::{Error, Result, types::Format};
use async_trait::async_trait;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;
use bytes::Bytes;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// S3 storage backend for genomic data files.
pub struct S3Storage {
    client: Client,
    bucket: String,
    prefix: String,
    cache_dir: PathBuf,
    presign_expiry: Duration,
}

impl S3Storage {
    /// Create a new S3Storage instance.
    ///
    /// # Arguments
    ///
    /// * `bucket` - S3 bucket name
    /// * `prefix` - Key prefix (e.g., "genomics/samples/")
    /// * `cache_dir` - Local directory for caching index files
    /// * `presign_expiry_secs` - Presigned URL expiration time in seconds
    /// * `region` - Optional AWS region (uses SDK defaults if not specified)
    /// * `endpoint` - Optional custom endpoint URL (for S3-compatible services)
    pub async fn new(
        bucket: String,
        prefix: String,
        cache_dir: PathBuf,
        presign_expiry_secs: u64,
        region: Option<String>,
        endpoint: Option<String>,
    ) -> Result<Self> {
        // Build AWS config
        let mut config_loader = aws_config::from_env();

        if let Some(region) = region {
            config_loader = config_loader.region(aws_config::Region::new(region));
        }

        let sdk_config = config_loader.load().await;

        // Build S3 client with optional custom endpoint
        let mut s3_config = aws_sdk_s3::config::Builder::from(&sdk_config);
        if let Some(endpoint) = endpoint {
            s3_config = s3_config.endpoint_url(endpoint).force_path_style(true);
        }

        let client = Client::from_conf(s3_config.build());

        // Ensure cache directory exists
        fs::create_dir_all(&cache_dir)
            .await
            .map_err(|e| Error::Internal(format!("failed to create cache dir: {}", e)))?;

        Ok(Self {
            client,
            bucket,
            prefix,
            cache_dir,
            presign_expiry: Duration::from_secs(presign_expiry_secs),
        })
    }

    /// Construct the S3 key for a data file.
    fn s3_key(&self, id: &str, format: Format) -> String {
        let ext = Self::file_extension(format);
        if self.prefix.is_empty() {
            format!("{}.{}", id, ext)
        } else {
            let prefix = self.prefix.trim_end_matches('/');
            format!("{}/{}.{}", prefix, id, ext)
        }
    }

    /// Construct the S3 key for an index file.
    fn s3_index_key(&self, id: &str, format: Format, appended: bool) -> Option<String> {
        let idx_ext = Self::index_extension(format)?;
        let data_ext = Self::file_extension(format);

        let prefix = if self.prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", self.prefix.trim_end_matches('/'))
        };

        Some(if appended {
            // e.g., sample.bam.bai
            format!("{}{}.{}.{}", prefix, id, data_ext, idx_ext)
        } else {
            // e.g., sample.bai
            format!("{}{}.{}", prefix, id, idx_ext)
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

    /// Get the local cache path for a data file header.
    fn data_cache_path(&self, id: &str, format: Format) -> PathBuf {
        let ext = Self::file_extension(format);
        self.cache_dir.join(format!("{}.{}", id, ext))
    }

    /// Check if an S3 object exists.
    async fn object_exists(&self, key: &str) -> bool {
        self.client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .is_ok()
    }

    /// Download an S3 object to a local file.
    async fn download_object(&self, s3_key: &str, cache_path: &PathBuf) -> Result<()> {
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("S3 get_object failed: {}", e)))?;

        let body = response
            .body
            .collect()
            .await
            .map_err(|e| Error::Internal(format!("S3 read body failed: {}", e)))?;

        let mut file = fs::File::create(cache_path)
            .await
            .map_err(|e| Error::Internal(format!("create cache file failed: {}", e)))?;

        file.write_all(&body.into_bytes())
            .await
            .map_err(|e| Error::Internal(format!("write cache file failed: {}", e)))?;

        Ok(())
    }

    /// Generate a presigned URL for an S3 object.
    async fn generate_presigned_url(
        &self,
        key: &str,
        range: Option<&ByteRange>,
    ) -> Result<String> {
        let presign_config = PresigningConfig::builder()
            .expires_in(self.presign_expiry)
            .build()
            .map_err(|e| Error::Internal(format!("presign config error: {}", e)))?;

        let mut request = self.client.get_object().bucket(&self.bucket).key(key);

        // Add Range header if byte range specified
        if let Some(r) = range {
            let range_header = match r.end {
                Some(end) => format!("bytes={}-{}", r.start, end),
                None => format!("bytes={}-", r.start),
            };
            request = request.range(range_header);
        }

        let presigned = request
            .presigned(presign_config)
            .await
            .map_err(|e| Error::Internal(format!("presign failed: {}", e)))?;

        Ok(presigned.uri().to_string())
    }
}

#[async_trait]
impl Storage for S3Storage {
    async fn exists(&self, id: &str, format: Format) -> Result<bool> {
        let key = self.s3_key(id, format);
        Ok(self.object_exists(&key).await)
    }

    async fn file_info(&self, id: &str, format: Format) -> Result<FileInfo> {
        let key = self.s3_key(id, format);

        let head = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|_| Error::NotFound(id.to_string()))?;

        let size = head.content_length().unwrap_or(0) as u64;

        // Check if index exists (try both naming conventions)
        let has_index = if let Some(appended_key) = self.s3_index_key(id, format, true) {
            if self.object_exists(&appended_key).await {
                true
            } else if let Some(replaced_key) = self.s3_index_key(id, format, false) {
                self.object_exists(&replaced_key).await
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

    fn data_url(&self, id: &str, format: Format, range: Option<ByteRange>) -> String {
        // Generate presigned URL for direct S3 access
        // This is synchronous in the trait but we need async AWS SDK
        // Use block_in_place to call async from sync context
        let key = self.s3_key(id, format);

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.generate_presigned_url(&key, range.as_ref())
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to generate presigned URL: {}", e);
                        format!("error://presign-failed?reason={}", e)
                    })
            })
        })
    }

    async fn read_bytes(
        &self,
        id: &str,
        format: Format,
        range: Option<ByteRange>,
    ) -> Result<Bytes> {
        let key = self.s3_key(id, format);

        let mut request = self.client.get_object().bucket(&self.bucket).key(&key);

        if let Some(ref r) = range {
            let range_header = match r.end {
                Some(end) => format!("bytes={}-{}", r.start, end),
                None => format!("bytes={}-", r.start),
            };
            request = request.range(range_header);
        }

        let response = request
            .send()
            .await
            .map_err(|_| Error::NotFound(id.to_string()))?;

        let body = response
            .body
            .collect()
            .await
            .map_err(|e| Error::Internal(format!("S3 read failed: {}", e)))?;

        Ok(body.into_bytes())
    }

    async fn index_path(&self, id: &str, format: Format) -> Result<Option<PathBuf>> {
        // Try appended index first (e.g., sample.bam.bai)
        if let Some(s3_key) = self.s3_index_key(id, format, true) {
            let cache_path = self.index_cache_path(id, format, true);

            // Check cache first
            if cache_path.exists() {
                return Ok(Some(cache_path));
            }

            // Check if exists in S3 and download
            if self.object_exists(&s3_key).await {
                self.download_object(&s3_key, &cache_path).await?;
                return Ok(Some(cache_path));
            }
        }

        // Try replaced extension (e.g., sample.bai)
        if let Some(s3_key) = self.s3_index_key(id, format, false) {
            let cache_path = self.index_cache_path(id, format, false);

            if cache_path.exists() {
                return Ok(Some(cache_path));
            }

            if self.object_exists(&s3_key).await {
                self.download_object(&s3_key, &cache_path).await?;
                return Ok(Some(cache_path));
            }
        }

        Ok(None)
    }

    fn file_path(&self, id: &str, format: Format) -> PathBuf {
        // Return path in cache directory
        // Note: The file may not exist locally yet - callers should ensure
        // they download it first if needed (e.g., via read_bytes or index_path)
        self.data_cache_path(id, format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s3_key_no_prefix() {
        let key = format!("{}.{}", "sample1", "bam");
        assert_eq!(key, "sample1.bam");
    }

    #[test]
    fn test_s3_key_with_prefix() {
        let prefix = "genomics/samples";
        let id = "sample1";
        let key = format!("{}/{}.bam", prefix, id);
        assert_eq!(key, "genomics/samples/sample1.bam");
    }

    #[test]
    fn test_file_extensions() {
        assert_eq!(S3Storage::file_extension(Format::Bam), "bam");
        assert_eq!(S3Storage::file_extension(Format::Cram), "cram");
        assert_eq!(S3Storage::file_extension(Format::Vcf), "vcf.gz");
        assert_eq!(S3Storage::file_extension(Format::Bcf), "bcf");
        assert_eq!(S3Storage::file_extension(Format::Fasta), "fa");
        assert_eq!(S3Storage::file_extension(Format::Fastq), "fq.gz");
    }

    #[test]
    fn test_index_extensions() {
        assert_eq!(S3Storage::index_extension(Format::Bam), Some("bai"));
        assert_eq!(S3Storage::index_extension(Format::Cram), Some("crai"));
        assert_eq!(S3Storage::index_extension(Format::Vcf), Some("tbi"));
        assert_eq!(S3Storage::index_extension(Format::Bcf), Some("csi"));
        assert_eq!(S3Storage::index_extension(Format::Fasta), Some("fai"));
        assert_eq!(S3Storage::index_extension(Format::Fastq), None);
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
