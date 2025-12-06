# lrmdns - Authoritative DNS Server Implementation Plan

## Project Goal
Build a lightweight, authoritative-only DNS server that responds to queries for configured zones with standard DNS record types.

## Technology Stack
- **Language**: Rust (for safety, performance, and strong typing)
- **DNS Library**: hickory-dns (formerly trust-dns) for protocol handling
- **Async Runtime**: tokio for concurrent request handling
- **Config Parsing**: serde with serde_yaml for configuration
- **Testing**: Built-in Rust testing + integration tests with real DNS queries

## Phase 1: Foundation (MVP)

### 1.1 Project Setup
- Initialize Rust project with Cargo
- Add dependencies: tokio, hickory-dns, serde, serde_yaml, tracing
- Set up project structure:
  ```
  src/
    main.rs           # Entry point
    server.rs         # UDP/TCP listeners
    protocol.rs       # DNS message handling
    zone.rs           # Zone data structures
    config.rs         # Configuration loading
  ```

### 1.2 Configuration System
- Define configuration schema (YAML format)
- Implement config file parser
- Support:
  - Server listen address/port
  - Zone file paths
  - Worker thread count
  - Log level

### 1.3 Zone Data Structure
- Design in-memory zone storage
  - `Zone` struct with origin, SOA, and record map
  - `RecordSet` for grouping records by name and type
- Implement zone lookup by domain name (longest suffix match)
- Support basic record types: SOA, NS, A, AAAA

### 1.4 Zone File Parser
- Implement RFC 1035 zone file parser OR use hickory-dns zone parser
- Parse directives: $ORIGIN, $TTL
- Parse resource records: SOA, NS, A, AAAA
- Validate zone data (must have SOA, at least one NS)
- Load all configured zones into memory on startup

### 1.5 UDP Server
- Create tokio UDP socket listener on port 53
- Receive DNS query packets
- Parse DNS wire format using hickory-dns
- Route to query processor
- Send DNS response packets
- Handle basic errors (FORMERR for invalid packets)

### 1.6 Query Processor
- Implement query resolution logic:
  1. Match QNAME to authoritative zone
  2. Lookup record in zone data
  3. Build response with appropriate flags (QR=1, AA=1)
  4. Populate answer section or return NXDOMAIN
- Handle QCLASS IN (Internet) only
- Support query types: A, AAAA, NS, SOA

### 1.7 Basic Logging
- Use `tracing` crate for structured logging
- Log:
  - Server startup/shutdown
  - Zone loading success/errors
  - Query requests (QNAME, QTYPE, source IP)
  - Response codes

### 1.8 Testing MVP
- Unit tests for zone parsing
- Unit tests for query matching
- Integration test: start server, send DNS query, verify response
- Manual testing with `dig` command

## Phase 2: Core Features

### 2.1 TCP Support
- Create tokio TCP listener on port 53
- Handle DNS over TCP (length prefix + message)
- Share query processor with UDP handler
- Handle connection lifecycle

### 2.2 Additional Record Types
- Add support for CNAME records
  - Implement CNAME chain resolution
  - Include both CNAME and target A/AAAA in response
- Add support for MX records
- Add support for TXT records
- Update zone parser for new types

### 2.3 Response Completeness
- Add authority section (NS records for negative responses)
- Add SOA record in authority section for NXDOMAIN
- Implement additional section (glue records for NS)
- Handle DNS name compression in responses

### 2.4 EDNS0 Support
- Parse EDNS0 OPT pseudo-record from queries
- Support larger UDP responses (>512 bytes)
- Set UDP payload size in responses
- Set TC (truncated) flag when response exceeds limit

### 2.5 Error Handling
- Return REFUSED for queries outside managed zones
- Return SERVFAIL for internal errors
- Return FORMERR for malformed queries
- Graceful handling of zone load failures

## Phase 3: Production Readiness

### 3.1 Zone Reloading
- Implement signal handler (SIGHUP)
- Reload zone files without server restart
- Atomic zone replacement (parse new, swap in memory)
- Log reload success/failure

### 3.2 Metrics and Monitoring
- Track query statistics:
  - Total queries
  - Queries by type
  - Responses by code (NOERROR, NXDOMAIN, etc.)
  - Error counts
- Expose metrics via log output or Prometheus endpoint
- Track response latency

### 3.3 Privilege Management
- Bind to port 53 as root
- Drop privileges to unprivileged user after binding
- Document systemd service configuration with capabilities

### 3.4 Rate Limiting
- Implement per-source-IP rate limiting
- Configurable queries per second threshold
- Return REFUSED or drop packets when limit exceeded
- Prevent resource exhaustion attacks

### 3.5 Comprehensive Testing
- Full unit test coverage for protocol handling
- Integration tests for all record types
- Fuzz testing for DNS packet parser
- Load testing with multiple concurrent queries
- Chaos testing (zone reload during queries)

## Phase 4: Advanced Features

### 4.1 Wildcard Records
- Support `*` in zone files
- Implement wildcard matching logic
- Return synthesized responses for wildcard matches

### 4.2 Zone Transfer (AXFR)
- Implement AXFR query handling
- Stream entire zone over TCP
- Add ACL for AXFR authorization
- Log zone transfer requests

### 4.3 Additional Record Types
- PTR (for reverse DNS zones)
- SRV (service records)
- CAA (certificate authority authorization)

### 4.4 Alternative Zone Format
- Support JSON or YAML zone files (in addition to RFC 1035)
- Easier for programmatic generation
- Schema validation

### 4.5 Management API
- Optional HTTP API for:
  - Zone reload trigger
  - Metrics retrieval
  - Health check endpoint
- Authentication for API access

## Phase 5: DNSSEC (Future)

### 5.1 Signing
- Offline zone signing with dnssec-signzone
- Load pre-signed zones with RRSIG, DNSKEY, NSEC/NSEC3

### 5.2 Online Signing
- Runtime DNSSEC signing
- Key management and rotation
- NSEC3 for authenticated denial of existence

## Deployment Plan

### Files and Directories
```
/usr/local/bin/lrmdns              # Binary
/etc/lrmdns/lrmdns.yaml            # Main config
/etc/lrmdns/zones/                 # Zone files directory
/var/log/lrmdns/                   # Logs (if not using journald)
/etc/systemd/system/lrmdns.service # Systemd unit
```

### Systemd Service
```ini
[Unit]
Description=lrmdns Authoritative DNS Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/lrmdns --config /etc/lrmdns/lrmdns.yaml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
User=lrmdns
Group=lrmdns
AmbientCapabilities=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
```

### Initial Configuration
```yaml
server:
  listen: "0.0.0.0:53"
  workers: 4
  log_level: info

zones:
  - name: example.com
    file: /etc/lrmdns/zones/example.com.zone
```

## Success Criteria

- [ ] Server starts and binds to port 53 (UDP + TCP)
- [ ] Successfully loads zone files on startup
- [ ] Responds to A, AAAA, NS, SOA, CNAME, MX, TXT queries
- [ ] Returns NXDOMAIN for non-existent names
- [ ] Returns REFUSED for non-authoritative queries
- [ ] Handles 10,000+ queries per second
- [ ] Zero memory leaks under load
- [ ] Zone reload without downtime
- [ ] Passes DNS compliance tests (e.g., DNSViz)
- [ ] Production deployment with 99.9% uptime

## Timeline Estimates

- **Phase 1 (MVP)**: Foundation for basic authoritative responses
- **Phase 2 (Core)**: Complete DNS server with all common record types
- **Phase 3 (Production)**: Hardened for production deployment
- **Phase 4 (Advanced)**: Extended features for specific use cases
- **Phase 5 (DNSSEC)**: Full cryptographic authentication support

## Open Questions

1. Should we use hickory-dns for full protocol handling or implement our own minimal parser?
2. JSON/YAML zone format vs. RFC 1035 format - support both or choose one?
3. Built-in DNSSEC support or rely on external signing tools?
4. Embedded metrics vs. external monitoring integration?
5. Single binary vs. separate tools for zone validation?
