# htsgetr

A Rust implementation of the [htsget protocol](https://samtools.github.io/hts-specs/htsget.html) (v1.3) for serving genomic data over HTTP. Built with [noodles](https://github.com/zaeleus/noodles) for bioinformatics I/O and [axum](https://github.com/tokio-rs/axum) for the web server.

## Features

- **htsget 1.3 compliant** - GET/POST endpoints for reads and variants
- **Multiple formats** - BAM, CRAM, VCF, BCF via noodles
- **Extensions** - FASTA/FASTQ support beyond the spec
- **Multiple storage backends** - Local filesystem, S3, and HTTP/HTTPS
- **JWT authentication** - Optional Bearer token auth with JWKS/static keys
- **Python bindings** - PyO3 integration via maturin
- **Async** - Built on tokio for high concurrency

## Installation

### From source (Rust)

```bash
git clone https://github.com/bolu-atx/htsgetr.git
cd htsgetr
cargo install --path .
```

### Python

```bash
pip install maturin
maturin develop --features python
```

## Quick Start

```bash
# Create a data directory with some BAM/VCF files
mkdir -p data
cp /path/to/sample.bam data/
cp /path/to/sample.bam.bai data/

# Start the server
htsgetr --data-dir ./data

# Query reads
curl http://localhost:8080/reads/sample
```

## Usage

### Server

```bash
# Start server with default settings
htsgetr --data-dir /path/to/data

# Custom host/port
htsgetr --host 0.0.0.0 --port 8080 --data-dir /path/to/data

# With explicit base URL (for proxied setups)
htsgetr --data-dir /path/to/data --base-url https://example.com/htsget
```

### Configuration

| Environment Variable | CLI Flag | Default | Description |
|---------------------|----------|---------|-------------|
| `HTSGET_HOST` | `--host` | `0.0.0.0` | Bind address |
| `HTSGET_PORT` | `--port` | `8080` | Listen port |
| `HTSGET_DATA_DIR` | `--data-dir` | `./data` | Directory containing genomic files |
| `HTSGET_BASE_URL` | `--base-url` | auto | Base URL for ticket URLs |
| `HTSGET_CORS` | `--cors` | `true` | Enable CORS |
| `HTSGET_STORAGE` | `--storage` | `local` | Storage backend: `local`, `s3`, or `http` |
| `RUST_LOG` | `--log-level` | `info` | Log level |

#### S3 Storage

```bash
cargo build --features s3

HTSGET_STORAGE=s3 \
HTSGET_S3_BUCKET=my-genomics-bucket \
HTSGET_S3_PREFIX=samples/ \
htsgetr
```

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `HTSGET_S3_BUCKET` | required | S3 bucket name |
| `HTSGET_S3_REGION` | auto | AWS region (uses AWS_REGION if not set) |
| `HTSGET_S3_PREFIX` | `""` | Key prefix for files |
| `HTSGET_S3_ENDPOINT` | - | Custom endpoint (for MinIO, LocalStack) |
| `HTSGET_PRESIGNED_URL_EXPIRY` | `3600` | Presigned URL TTL in seconds |
| `HTSGET_CACHE_DIR` | `/tmp/htsgetr-cache` | Local cache for index files |

#### HTTP Storage

```bash
cargo build --features http

HTSGET_STORAGE=http \
HTSGET_HTTP_BASE_URL=https://files.example.com/genomics/ \
htsgetr
```

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `HTSGET_HTTP_BASE_URL` | required | Base URL for data files |
| `HTSGET_HTTP_INDEX_BASE_URL` | - | Base URL for index files (defaults to data URL) |

#### Authentication

Enable JWT/Bearer token authentication by building with the `auth` feature:

```bash
cargo build --features auth

HTSGET_AUTH_ENABLED=true \
HTSGET_AUTH_ISSUER=https://auth.example.com \
htsgetr --features auth
```

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `HTSGET_AUTH_ENABLED` | `false` | Enable authentication |
| `HTSGET_AUTH_ISSUER` | - | JWT issuer URL (JWKS fetched from `{issuer}/.well-known/jwks.json`) |
| `HTSGET_AUTH_AUDIENCE` | - | Expected `aud` claim |
| `HTSGET_AUTH_JWKS_URL` | auto | Explicit JWKS URL (overrides issuer-derived URL) |
| `HTSGET_AUTH_PUBLIC_KEY` | - | Static RSA/EC PEM public key (alternative to JWKS) |
| `HTSGET_AUTH_PUBLIC_ENDPOINTS` | `/,/service-info` | Comma-separated paths that don't require auth |
| `HTSGET_DATA_URL_SECRET` | generated | HMAC secret for signing data URLs |
| `HTSGET_DATA_URL_EXPIRY` | `3600` | Signed data URL TTL in seconds |

When auth is enabled:
- Public endpoints (root, service-info) don't require authentication
- All other endpoints require a valid `Authorization: Bearer <token>` header
- Data URLs in tickets are HMAC-signed with expiry to prevent unauthorized access

### Data Directory Structure

Place files in the data directory with standard extensions:

```
data/
├── sample1.bam
├── sample1.bam.bai
├── sample2.vcf.gz
├── sample2.vcf.gz.tbi
├── reference.fa
└── reference.fa.fai
```

## API Reference

### Reads Endpoint

```bash
# Get all reads
curl http://localhost:8080/reads/sample1

# Get reads for a region
curl "http://localhost:8080/reads/sample1?referenceName=chr1&start=0&end=1000000"

# Header only
curl "http://localhost:8080/reads/sample1?class=header"

# Request CRAM format
curl "http://localhost:8080/reads/sample1?format=CRAM"

# POST with multiple regions
curl -X POST http://localhost:8080/reads/sample1 \
  -H "Content-Type: application/json" \
  -d '{
    "format": "BAM",
    "regions": [
      {"referenceName": "chr1", "start": 0, "end": 1000000},
      {"referenceName": "chr2", "start": 0, "end": 500000}
    ]
  }'
```

### Variants Endpoint

```bash
# Get all variants
curl http://localhost:8080/variants/sample2

# Get variants for a region
curl "http://localhost:8080/variants/sample2?referenceName=chr1&start=0&end=1000000"
```

### Sequences Endpoint (Extension)

```bash
# Get FASTA sequence
curl http://localhost:8080/sequences/reference
```

### Service Info

```bash
curl http://localhost:8080/service-info
```

### Response Format

Successful responses return a JSON ticket per the htsget spec:

```json
{
  "htsget": {
    "format": "BAM",
    "urls": [
      {
        "url": "http://localhost:8080/data/reads/sample1",
        "class": "body"
      }
    ]
  }
}
```

Error responses:

```json
{
  "htsget": {
    "error": "NotFound",
    "message": "not found: sample1"
  }
}
```

## Python Bindings

```python
from htsgetr import HtsgetServer, HtsgetClient

# Start a server
server = HtsgetServer("/path/to/data", port=8080)
server.run()  # Blocking

# Use the client
client = HtsgetClient("http://localhost:8080")
ticket = client.reads("sample1", reference_name="chr1", start=0, end=1000000)
```

## Roadmap

- [x] Server scaffold with axum
- [x] htsget 1.3 types and error handling
- [x] Local filesystem storage backend
- [x] Reads endpoint (BAM/CRAM)
- [x] Variants endpoint (VCF/BCF)
- [x] Sequences endpoint (FASTA/FASTQ extension)
- [x] Index-based byte range queries (BAI, TBI, CSI)
- [x] S3 storage backend with pre-signed URLs
- [x] HTTP/HTTPS storage backend
- [x] JWT/Bearer token authentication
- [ ] CRAM reference resolution
- [ ] GCS storage backend
- [ ] Python bindings (full implementation)
- [ ] Docker image

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Related Projects

- [htsget-rs](https://github.com/umccr/htsget-rs) - Another Rust htsget implementation
- [noodles](https://github.com/zaeleus/noodles) - Bioinformatics I/O libraries in Rust
- [htslib](https://github.com/samtools/htslib) - C library for HTS formats

## License

MIT License - see [LICENSE](LICENSE) for details.
