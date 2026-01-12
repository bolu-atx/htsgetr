# Getting Started with htsgetr

This guide walks you through setting up htsgetr for common scenarios.

## What is htsgetr?

htsgetr is a server that implements the [GA4GH htsget protocol](https://samtools.github.io/hts-specs/htsget.html) for streaming genomic data. Instead of downloading entire BAM/VCF files, clients request specific regions and get back URLs pointing to just the bytes they need.

**How it works:**
1. Client requests data: `GET /reads/sample1?referenceName=chr1&start=0&end=1000000`
2. Server returns a "ticket" with URLs pointing to the relevant byte ranges
3. Client fetches data from those URLs and concatenates the results

## Installation

```bash
# Clone and build
git clone https://github.com/bolu-atx/htsgetr.git
cd htsgetr
cargo build --release

# With optional features
cargo build --release --features s3      # S3 storage
cargo build --release --features http    # HTTP storage
cargo build --release --features auth    # JWT authentication
cargo build --release --features s3,auth # Multiple features
```

## Scenario 1: Serving Local Files

The simplest setup - serve BAM/VCF files from a local directory.

**1. Prepare your data directory:**

```bash
mkdir -p /data/genomics
cp sample1.bam sample1.bam.bai /data/genomics/
cp variants.vcf.gz variants.vcf.gz.tbi /data/genomics/
```

Index files (`.bai`, `.tbi`, `.csi`) must be present for region queries to work.

**2. Start the server:**

```bash
htsgetr --data-dir /data/genomics --port 8080
```

**3. Test it:**

```bash
# Get ticket for all reads
curl http://localhost:8080/reads/sample1

# Get ticket for a specific region
curl "http://localhost:8080/reads/sample1?referenceName=chr1&start=0&end=1000000"

# Fetch the actual data (follow the URLs in the ticket)
curl http://localhost:8080/data/bam/sample1 > sample1.bam
```

## Scenario 2: Serving from S3

Serve files directly from an S3 bucket with presigned URLs.

**1. Build with S3 support:**

```bash
cargo build --release --features s3
```

**2. Organize your bucket:**

```
s3://my-genomics-bucket/
├── samples/
│   ├── sample1.bam
│   ├── sample1.bam.bai
│   ├── sample2.bam
│   └── sample2.bam.bai
```

**3. Start the server:**

```bash
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...
export AWS_REGION=us-west-2

HTSGET_STORAGE=s3 \
HTSGET_S3_BUCKET=my-genomics-bucket \
HTSGET_S3_PREFIX=samples/ \
HTSGET_BASE_URL=https://htsget.example.com \
htsgetr
```

Clients get presigned S3 URLs in their tickets, so they download directly from S3.

**Using with MinIO or LocalStack:**

```bash
HTSGET_STORAGE=s3 \
HTSGET_S3_BUCKET=test-bucket \
HTSGET_S3_ENDPOINT=http://localhost:9000 \
htsgetr
```

## Scenario 3: Proxying an Existing HTTP Server

If you already have files on an HTTP server but want to add htsget support.

**1. Build with HTTP support:**

```bash
cargo build --release --features http
```

**2. Start the server:**

```bash
HTSGET_STORAGE=http \
HTSGET_HTTP_BASE_URL=https://files.example.com/genomics/ \
htsgetr
```

Requests for `/reads/sample1` will look for files at `https://files.example.com/genomics/sample1.bam`.

**Separate index server:**

If your index files are on a different server:

```bash
HTSGET_STORAGE=http \
HTSGET_HTTP_BASE_URL=https://data.example.com/bam/ \
HTSGET_HTTP_INDEX_BASE_URL=https://index.example.com/bam/ \
htsgetr
```

## Scenario 4: Adding Authentication

Protect your endpoints with JWT authentication.

**1. Build with auth support:**

```bash
cargo build --release --features auth
```

**2. Configure with your identity provider:**

```bash
# Using Auth0, Okta, Keycloak, or any OIDC provider
HTSGET_AUTH_ENABLED=true \
HTSGET_AUTH_ISSUER=https://your-tenant.auth0.com/ \
HTSGET_AUTH_AUDIENCE=htsget-api \
htsgetr
```

The server automatically fetches public keys from `{issuer}/.well-known/jwks.json`.

**3. Making authenticated requests:**

```bash
# Get a token from your identity provider
TOKEN=$(curl -X POST https://your-tenant.auth0.com/oauth/token \
  -d "grant_type=client_credentials&client_id=...&client_secret=...&audience=htsget-api" \
  | jq -r .access_token)

# Use the token
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/reads/sample1
```

**Using a static public key (no JWKS):**

```bash
HTSGET_AUTH_ENABLED=true \
HTSGET_AUTH_PUBLIC_KEY="$(cat public_key.pem)" \
htsgetr
```

**Public endpoints:**

By default, `/` and `/service-info` don't require auth. Customize with:

```bash
HTSGET_AUTH_PUBLIC_ENDPOINTS=/,/service-info,/health
```

## Scenario 5: Production Deployment

A typical production setup behind a reverse proxy.

**1. Create a systemd service (`/etc/systemd/system/htsgetr.service`):**

```ini
[Unit]
Description=htsget server
After=network.target

[Service]
Type=simple
User=htsget
Environment=HTSGET_HOST=127.0.0.1
Environment=HTSGET_PORT=8080
Environment=HTSGET_DATA_DIR=/data/genomics
Environment=HTSGET_BASE_URL=https://htsget.example.com
Environment=RUST_LOG=info
ExecStart=/usr/local/bin/htsgetr
Restart=always

[Install]
WantedBy=multi-user.target
```

**2. Configure nginx:**

```nginx
server {
    listen 443 ssl;
    server_name htsget.example.com;

    ssl_certificate /etc/ssl/certs/htsget.crt;
    ssl_certificate_key /etc/ssl/private/htsget.key;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

**3. Start:**

```bash
sudo systemctl enable --now htsgetr
```

## Troubleshooting

**"NotFound" error for files that exist:**
- Check file extensions match expected format (`.bam`, `.vcf.gz`, etc.)
- Ensure the file ID in the URL matches the filename without extension
- For S3: verify the prefix doesn't have trailing/leading slashes issues

**Region queries return the whole file:**
- Index files must be present (`.bai` for BAM, `.tbi` or `.csi` for VCF)
- Index must be for the correct file (regenerate with `samtools index` if unsure)

**Authentication failing:**
- Check `HTSGET_AUTH_ISSUER` matches the `iss` claim in your JWTs
- Verify the JWKS endpoint is accessible from the server
- Enable debug logging: `RUST_LOG=debug`

**S3 presigned URLs expiring:**
- Increase `HTSGET_PRESIGNED_URL_EXPIRY` (default: 3600 seconds)
- Ensure client clocks are synchronized

## Next Steps

- [API Reference](../README.md#api-reference) - Full endpoint documentation
- [htsget spec](https://samtools.github.io/hts-specs/htsget.html) - Protocol specification
- [noodles](https://github.com/zaeleus/noodles) - Underlying bioinformatics library
