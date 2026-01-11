# htsgetr Roadmap

Feature roadmap for htsgetr, prioritized by impact and implementation dependencies.

## Phase 1: Protocol Compliance

Core htsget protocol features needed for full compliance.

### POST Request Support
- Add POST endpoint handlers for `/reads` and `/variants`
- Accept JSON body with query parameters per htsget spec
- Share query logic with existing GET handlers

### CORS Support
- Add tower-http CORS middleware
- Configurable allowed origins
- Support preflight OPTIONS requests

### TLS Support
- Add rustls/native-tls for HTTPS
- Configure separate TLS for ticket and data servers
- Add `--tls-cert` and `--tls-key` CLI options

## Phase 2: Cloud Storage

Enable serving data from cloud storage backends.

### S3 Storage Backend
- Add `S3Storage` implementation of `Storage` trait
- Generate presigned URLs for data tickets
- Support AWS credentials via environment/IAM
- Add `--storage s3://bucket` CLI option
- Dependencies: `aws-sdk-s3`

### HTTP/URL Storage Backend
- Add `UrlStorage` implementation for remote HTTP files
- Proxy or redirect to upstream URLs
- Support range requests to remote servers
- Add `--storage https://example.com/data` CLI option

### Storage Abstraction Improvements
- Support multiple storage backends simultaneously
- Regex-based ID to storage mapping
- Storage selection based on request path prefix

## Phase 3: Configuration

Flexible configuration system.

### TOML Configuration Files
- Add `--config` CLI option
- Support all current CLI options in TOML
- Hot-reload configuration (optional)
- Dependencies: `toml`, `serde`

### Regex-based ID Resolution
- Map request IDs to file paths using regex capture groups
- Support substitution strings: `$1`, `$name`
- Enable complex directory structures

### Service Info Configuration
- Configurable `/service-info` response
- Support GA4GH service-info spec fields
- Custom metadata fields

## Phase 4: Security

Security features for production deployments.

### Crypt4GH Encryption Support
- Decrypt Crypt4GH-encrypted files on the fly
- Support key configuration
- Edit encrypted byte ranges correctly
- Dependencies: `crypt4gh`

### Authentication Middleware
- Optional JWT/Bearer token validation
- Configurable auth providers
- Per-endpoint auth requirements

## Phase 5: Serverless

Cloud-native deployment options.

### AWS Lambda Support
- Add `htsgetr-lambda` binary target
- Integrate with `lambda_http` runtime
- API Gateway compatible responses
- Separate ticket and data Lambda functions
- Dependencies: `lambda_runtime`, `lambda_http`

### Container Improvements
- Optimized Docker image
- Helm chart for Kubernetes
- AWS CDK/Terraform examples

## Phase 6: Performance & Testing

Production hardening.

### Benchmarks
- Add criterion.rs benchmarks
- Benchmark index parsing
- Benchmark byte range calculations
- CI benchmark regression tracking

### Integration Test Suite
- End-to-end protocol compliance tests
- Test against htsget reference test data
- Fuzz testing for query parsing

### Observability
- Structured logging (tracing)
- Prometheus metrics endpoint
- OpenTelemetry integration

---

## Current Advantages to Maintain

Features htsgetr has that alternatives don't:

- **FASTA/FASTQ support** - `/sequences` endpoint
- **Python bindings** - PyO3 integration for Python users
- **Simple architecture** - single crate, easy to understand and deploy

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for how to help implement these features.
