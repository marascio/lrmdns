# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

lrmdns is an **authoritative-only DNS server** written in Rust. It does NOT perform recursive resolution - it only serves zones that are explicitly configured. The server supports both UDP and TCP protocols, handles standard DNS record types including DNSSEC records, and provides AXFR zone transfers.

## Development Workflow

### Always Work in Branches
- **NEVER commit directly to master**
- Always create a feature branch before making changes
- Merge to master only after testing
- Delete the feature branch after merging

### Pre-Commit Checklist
Before committing any work, always verify:
1. Build debug without warnings: `cargo build`
2. Build release without warnings: `cargo build --release`
3. Run Rust unit tests in debug mode: `cargo test`
4. Run Rust unit tests in release mode: `cargo test --release`
5. Run full integration test suite: `cd it && ./run-tests.sh`

### Git Commit Messages
- Do NOT use emojis or unicode characters in commit messages
- Always include attribution to Claude for co-authoring changes.

## Building and Testing

### Build Commands
```bash
# Debug build
cargo build

# Release build (for deployment)
cargo build --release

# Run server with config file
cargo run -- lrmdns.yaml
# or
./target/release/lrmdns lrmdns.yaml
```

### Test Commands
```bash
# Rust unit tests
cargo test

# Integration tests (BATS-based)
cd it
./run-tests.sh                    # All tests
./run-tests.sh --parallel         # Parallel execution
bats/bats-core/bin/bats tests/01-basic-queries.bats  # Single test file

# Validate integration test setup
cd it && ./scripts/validate-setup.sh
```

### Manual Testing with dig
```bash
# Start server (in one terminal)
./target/release/lrmdns lrmdns.yaml

# Query in another terminal
dig @127.0.0.1 -p 5353 www.example.com A
dig @127.0.0.1 -p 5353 example.com NS
dig @127.0.0.1 -p 5353 +dnssec example.com A  # DNSSEC query
```

### Signal Handling (Unix only)
```bash
# Reload zones without restart
kill -HUP <pid>

# Log current metrics
kill -USR1 <pid>
```

## Architecture

### Core Components

**Main Entry Point** (`src/main.rs`)
- Loads configuration from YAML file
- Initializes ZoneStore with all configured zones
- Creates QueryProcessor, Metrics, and RateLimiter
- Spawns concurrent UDP and TCP servers
- Sets up signal handlers for zone reload (SIGHUP) and metrics (SIGUSR1)
- Optionally starts HTTP API server for health/metrics endpoints

**ZoneStore** (`src/zone.rs`)
- In-memory storage using `HashMap<Name, Zone>`
- Each Zone contains: origin, SOA record, and `HashMap<Name, HashMap<RecordType, Vec<Record>>>`
- Wrapped in `Arc<RwLock<>>` for concurrent read access and reload capability
- Zone file parser handles RFC 1035 format with DNSSEC extensions
- Supports wildcards (`*.example.com`), `$ORIGIN`, `$TTL` directives

**QueryProcessor** (`src/protocol.rs`)
- Main query handling logic
- Looks up zone by finding the longest matching suffix of the query name
- Handles CNAME chain resolution automatically
- Returns proper response codes: NXDOMAIN, REFUSED, FORMERR, NOERROR
- Special handling for AXFR queries (marks them for TCP streaming)
- EDNS0 support with automatic RRSIG inclusion when DO flag is set

**DnsServer** (`src/server.rs`)
- Runs UDP and TCP servers concurrently via `tokio::try_join!`
- UDP: Standard 512-byte responses, EDNS0 for larger (up to 4096 bytes)
- TCP: Length-prefixed messages (2-byte length + DNS message), supports up to 65535 bytes
- AXFR over TCP: Streams entire zone as sequence of resource records
- Rate limiting per source IP (if configured)
- Metrics tracking for all queries

**Metrics** (`src/metrics.rs`)
- Atomic counters for total queries, UDP/TCP queries, NXDOMAIN, etc.
- Exposed via HTTP API at `/metrics` endpoint (JSON)
- Can be logged on demand via SIGUSR1 signal

**DNSSEC** (`src/dnssec.rs`)
- Validation utilities for DS digest, RRSIG time validity, NSEC denial
- Key tag computation (RFC 4034)
- Zone parser handles base64/hex encoding for DNSKEY, RRSIG, DS records
- Server does NOT perform online signing - zones must be pre-signed

**Rate Limiter** (`src/ratelimit.rs`)
- Token bucket algorithm per source IP
- Configurable queries-per-second limit
- Returns REFUSED for rate-limited queries

**HTTP API** (`src/api.rs`)
- `/health` - Health check endpoint
- `/metrics` - Query statistics in JSON format
- Optional, enabled via `server.api_listen` in config

### Data Flow

1. **Startup**: Load config → Parse zone files → Build ZoneStore → Start servers
2. **UDP Query**: Receive packet → Deserialize → QueryProcessor → Serialize → Send response
3. **TCP Query**: Accept connection → Read length-prefixed message → QueryProcessor → Send length-prefixed response
4. **AXFR**: Detect AXFR query → Stream all zone records over TCP connection
5. **SIGHUP**: Reload all zone files → Replace ZoneStore atomically with RwLock

### Key Design Decisions

**Authoritative Only**: Never performs recursive resolution or caching. Returns REFUSED for zones not configured.

**Async I/O**: Uses tokio for all network I/O. Each UDP query is handled in a separate tokio task. Each TCP connection gets its own task.

**Immutable Zones**: Zones are loaded into memory at startup. Reload is atomic replacement via RwLock. No dynamic updates (RFC 2136).

**CNAME Transparency**: QueryProcessor automatically chases CNAME chains and returns final A/AAAA records in the answer section (with CNAME in additional section).

**DNSSEC Passive**: Server serves pre-signed zones. RRSIG records are automatically included when EDNS0 DO flag is set. No online signing or validation.

## Configuration Format

```yaml
server:
  listen: "127.0.0.1:5353"       # Address:port for DNS server
  workers: 4                      # Number of tokio worker threads
  log_level: info                 # trace, debug, info, warn, error
  rate_limit: 100                 # Optional: queries per second per IP
  api_listen: "127.0.0.1:8080"   # Optional: HTTP API endpoint

zones:
  - name: example.com.            # Must end with dot
    file: zones/example.zone      # Path to zone file
```

## Zone File Format

Standard RFC 1035 format with extensions:

**Directives**: `$ORIGIN`, `$TTL`
**Required**: SOA record, at least one NS record
**Supported Record Types**: A, AAAA, NS, SOA, CNAME, MX, TXT, PTR, SRV, CAA, DNSKEY, RRSIG, NSEC, DS
**Wildcards**: `*.example.com` for wildcard matching
**DNSSEC**: Base64 for DNSKEY/RRSIG public keys and signatures, hex for DS digests

## Integration Testing

Integration tests are in the `it/` directory and use BATS (Bash Automated Testing System). Tests are completely independent of the Rust codebase and use external tools (dig, curl).

**Test Organization**:
- `00-09`: Core DNS functionality
- `10-19`: Server features (reload, metrics, etc.)
- `20-29`: Reserved for PCAP-based tests
- `30-39`: Reserved for external test suites (Deckard, FerretDataset)

**Test Isolation**: Each test auto-assigns a unique port (20000 + BATS_TEST_NUMBER) to support parallel execution.

**Key Helper Functions** (in `it/helpers/`):
- `start_server <config> [port]` - Starts server, auto-assigns port if not specified
- `cleanup_server` - Stops server and cleans up temp files
- `query_a <domain>` - Query A record
- `query_tcp <domain> <type>` - Query over TCP
- `get_rcode <domain> <type>` - Get response code

## Common Development Tasks

### Adding a New DNS Record Type
1. Add parsing logic in `zone.rs::parse_zone_file()` (look for the match on record type)
2. Handle in `protocol.rs::process_query()` if special behavior needed
3. Add integration test in `it/tests/01-basic-queries.bats` or relevant test file
4. Add test zone file in `it/fixtures/zones/` if needed

### Adding Server Configuration Option
1. Add field to `Config` struct in `src/config.rs`
2. Update validation in `Config::validate()`
3. Wire through `main.rs` to the appropriate component
4. Document in README.md configuration section

### Implementing a New Feature
1. Create feature branch: `git checkout -b feat/feature-name`
2. Make changes and add tests (both Rust unit tests and BATS integration tests)
3. Run full test suite: `cd it && ./run-tests.sh`
4. Build both debug and release: `cargo build && cargo build --release`
5. Commit with Co-Authored-By line
6. Merge to master: `git checkout master && git merge feat/feature-name --no-edit`
7. Delete branch: `git branch -d feat/feature-name`

## Known Limitations

- No online DNSSEC signing (zones must be pre-signed)
- No NSEC3 support (only NSEC for authenticated denial)
- No dynamic updates (RFC 2136)
- Zone reload requires SIGHUP signal (no auto-reload on file change)
- No caching (authoritative only)
- No recursive resolution

## Dependencies

**Core**:
- `tokio` - Async runtime
- `hickory-proto` - DNS protocol implementation (formerly trust-dns)
- `serde`/`serde_yaml` - Configuration parsing

**HTTP API**:
- `axum` - HTTP server framework

**DNSSEC**:
- `base64`, `hex` - Encoding/decoding
- `ring`, `sha2` - Cryptographic operations

**Testing**:
- BATS (git submodule in `it/bats/`)
- `dig` (from BIND tools)
- `nc` (netcat for port checking)
- `curl` (for API tests)
