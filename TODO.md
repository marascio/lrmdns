# TODO

## Bugs

- [ ] SSHFP record serialization produces FORMERR on Linux (hickory-proto issue)
  - Server correctly parses and stores SSHFP records
  - Serialization to wire format triggers FORMERR in dig on Ubuntu
  - Works correctly on macOS
  - SSHFP tests currently skipped on Linux in CI
  - Likely an issue with hickory-proto's SSHFP RData implementation

## Performance & Scalability

- [ ] Add benchmarking suite (compare against BIND, Knot, etc.)
- [ ] Implement query caching for frequently requested records
- [x] Add connection pooling/reuse for TCP queries
- [ ] Profile and optimize hot paths in query processing

## Features

- [ ] DNSSEC signing support (currently only serving signed records)
- [ ] Dynamic updates (RFC 2136)
- [ ] NOTIFY/IXFR for zone change notifications
- [ ] Response rate limiting (RRL) for DDoS mitigation enhancement
- [ ] Geographic load balancing (GeoDNS)

## Observability

- [ ] Structured logging with JSON output option
- [ ] Prometheus metrics export format
- [ ] OpenTelemetry tracing integration
- [ ] Query logging with configurable filters
- [ ] Statistics dashboard (web UI)

## Testing

- [ ] Fuzz testing for packet parser
- [x] Property-based testing for DNS protocol compliance
- [ ] Load testing scenarios
- [ ] Chaos engineering tests (network failures, etc.)
- [ ] CI/CD pipeline integration (GitHub Actions)
- [ ] Integration with Deckard DNS test suite
- [ ] Integration with FerretDataset test cases

## Operational

- [ ] Systemd service files
- [ ] Docker container image with multi-stage build
- [ ] Configuration validation tool (dry-run mode)
- [ ] Zone file syntax checker with better error messages
- [ ] Hot reload for configuration changes (not just zones)
- [ ] Graceful shutdown improvements (drain connections)

## Code Quality

- [ ] Comprehensive API documentation (rustdoc)
- [ ] Increase test coverage (particularly error paths)
- [ ] Dead code elimination audit
- [ ] Dependency audit and updates
- [ ] Code coverage reporting (tarpaulin or similar)

## Security

- [ ] Per-IP rate limiting enhancements
- [ ] Query source validation
- [ ] TSIG authentication for zone transfers
- [ ] Security audit of packet parsing code
- [ ] Hardening against DNS amplification attacks

## Documentation

- [ ] Architecture decision records (ADRs)
- [ ] Performance tuning guide
- [ ] Production deployment guide
- [ ] Zone signing tutorial
- [ ] Troubleshooting guide

## Future Phases

- [ ] NSEC3 support for authenticated denial
- [ ] Online DNSSEC signing capability
- [ ] Multi-master zone replication
- [ ] REST API for zone management
- [ ] Metrics aggregation and alerting
