# htsgetr

A Rust implementation of the [htsget protocol](https://samtools.github.io/hts-specs/htsget.html) (v1.3) for serving genomic data over HTTP. Built with [noodles](https://github.com/zaeleus/noodles) for bioinformatics I/O and [axum](https://github.com/tokio-rs/axum) for the web server.

## Features

- **htsget 1.3 compliant** - GET/POST endpoints for reads and variants
- **Multiple formats** - BAM, CRAM, VCF, BCF via noodles
- **Extensions** - FASTA/FASTQ support beyond the spec
- **Python bindings** - PyO3 integration via maturin
- **Async** - Built on tokio for high concurrency
- **Pluggable storage** - Local filesystem, with S3/GCS planned

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
| `RUST_LOG` | `--log-level` | `info` | Log level |

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
- [ ] Index-based byte range queries (BAI, TBI, CSI)
- [ ] CRAM reference resolution
- [ ] S3 storage backend with pre-signed URLs
- [ ] GCS storage backend
- [ ] OAuth2 bearer token authentication
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
