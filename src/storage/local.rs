use super::{ByteRange, FileInfo, Storage};
use crate::{Error, Result, types::Format};
use async_trait::async_trait;
use bytes::Bytes;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub struct LocalStorage {
    data_dir: PathBuf,
    base_url: String,
}

impl LocalStorage {
    pub fn new(data_dir: PathBuf, base_url: String) -> Self {
        Self { data_dir, base_url }
    }

    fn make_file_path(&self, id: &str, format: Format) -> PathBuf {
        let ext = match format {
            Format::Bam => "bam",
            Format::Cram => "cram",
            Format::Vcf => "vcf.gz",
            Format::Bcf => "bcf",
            Format::Fasta => "fa",
            Format::Fastq => "fq.gz",
        };
        self.data_dir.join(format!("{}.{}", id, ext))
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
}

#[async_trait]
impl Storage for LocalStorage {
    async fn exists(&self, id: &str, format: Format) -> Result<bool> {
        let path = self.make_file_path(id, format);
        Ok(path.exists())
    }

    async fn file_info(&self, id: &str, format: Format) -> Result<FileInfo> {
        let path = self.make_file_path(id, format);
        let metadata = fs::metadata(&path)
            .await
            .map_err(|_| Error::NotFound(id.to_string()))?;

        let has_index = if let Some(idx_ext) = Self::index_extension(format) {
            // Check both appended (file.bam.bai) and replaced (file.bai) conventions
            let appended_idx = PathBuf::from(format!("{}.{}", path.display(), idx_ext));
            let replaced_idx = path.with_extension(idx_ext);
            appended_idx.exists() || replaced_idx.exists()
        } else {
            false
        };

        Ok(FileInfo {
            id: id.to_string(),
            format,
            size: metadata.len(),
            has_index,
        })
    }

    fn data_url(&self, id: &str, format: Format, range: Option<ByteRange>) -> String {
        let base = format!("{}/data/{}/{}", self.base_url, format_path(format), id);
        if let Some(r) = range {
            match r.end {
                Some(end) => format!("{}?start={}&end={}", base, r.start, end),
                None => format!("{}?start={}", base, r.start),
            }
        } else {
            base
        }
    }

    async fn read_bytes(
        &self,
        id: &str,
        format: Format,
        range: Option<ByteRange>,
    ) -> Result<Bytes> {
        let path = self.make_file_path(id, format);
        let mut file = fs::File::open(&path)
            .await
            .map_err(|_| Error::NotFound(id.to_string()))?;

        let bytes = match range {
            Some(r) => {
                file.seek(std::io::SeekFrom::Start(r.start)).await?;
                let len = r.end.map(|e| e - r.start).unwrap_or(u64::MAX) as usize;
                let mut buf = vec![0u8; len.min(10 * 1024 * 1024)]; // Cap at 10MB
                let n = file.read(&mut buf).await?;
                buf.truncate(n);
                Bytes::from(buf)
            }
            None => {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).await?;
                Bytes::from(buf)
            }
        };

        Ok(bytes)
    }

    async fn index_path(&self, id: &str, format: Format) -> Result<Option<PathBuf>> {
        let path = self.make_file_path(id, format);
        if let Some(idx_ext) = Self::index_extension(format) {
            // Try appended index first (e.g., file.bam.bai)
            let appended_idx = PathBuf::from(format!("{}.{}", path.display(), idx_ext));
            if appended_idx.exists() {
                return Ok(Some(appended_idx));
            }

            // Try replaced extension (e.g., file.bai)
            let replaced_idx = path.with_extension(idx_ext);
            if replaced_idx.exists() {
                return Ok(Some(replaced_idx));
            }
        }
        Ok(None)
    }

    fn file_path(&self, id: &str, format: Format) -> PathBuf {
        self.make_file_path(id, format)
    }
}

fn format_path(format: Format) -> &'static str {
    match format {
        Format::Bam | Format::Cram => "reads",
        Format::Vcf | Format::Bcf => "variants",
        Format::Fasta | Format::Fastq => "sequences",
    }
}
