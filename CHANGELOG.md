# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Prepared repository for GitHub publication
- Added MIT license
- Added SECURITY.md with security policy and reporting guidelines
- Enhanced Cargo.toml with complete package metadata

## [0.1.0] - 2024-12-07

### Added

#### Core Features
- Authoritative-only DNS server implementation
- UDP and TCP server support
- Async I/O using tokio for high concurrency
- YAML configuration file support
- Structured logging with tracing framework
- Zone file parsing (RFC 1035 format)
- Hot reload capability via SIGHUP signal
- Metrics collection and HTTP API endpoint
- Per-IP rate limiting

#### DNS Record Types
- Standard record types: A, AAAA, NS, SOA, CNAME, MX, TXT, PTR, SRV, CAA
- DNSSEC record types: DNSKEY, RRSIG, NSEC, DS
- Additional record types: NAPTR, TLSA, SSHFP

#### DNS Protocol Features
- Proper DNS response codes: NXDOMAIN, REFUSED, FORMERR, NOERROR, NOTIMP
- CNAME chain resolution
- Wildcard record support (*.example.com)
- EDNS0 support with up to 4096 byte UDP responses
- DNSSEC-aware responses (DO flag handling)
- AXFR zone transfers over TCP
- Multi-line SOA record support with parentheses
- TCP connection pooling and reuse

#### Testing
- Comprehensive unit tests (122 tests)
- Property-based testing with proptest (17 property tests)
- Integration test suite using BATS (69 tests)
- Parallel test execution support
- Pre-commit hooks for code quality (rustfmt, clippy)

#### Documentation
- Complete README with installation and usage instructions
- TODO.md for tracking future development
- PERFORMANCE.md with benchmarking results
- CLAUDE.md documenting AI-assisted development
- Example zone files and configurations

### Technical Details
- Rust edition 2021
- Hickory-proto (formerly trust-dns) for DNS protocol handling
- Tokio async runtime
- Immutable zone storage with Arc<RwLock> for concurrent access
- Atomic zone reload without downtime

[Unreleased]: https://github.com/marascio/lrmdns/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/marascio/lrmdns/releases/tag/v0.1.0
