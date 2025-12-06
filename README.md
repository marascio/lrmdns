# lrmdns - Lightweight Authoritative DNS Server

A simple, fast, and reliable authoritative-only DNS server written in Rust.

## Overview

lrmdns is an authoritative DNS server that responds to queries for domains it manages. It does NOT perform recursive resolution or caching - it only serves zones that are explicitly configured.

## Features (Phase 1 - MVP)

✅ **Authoritative DNS responses** for configured zones
✅ **UDP server** on configurable port
✅ **Standard DNS record types**: A, AAAA, NS, SOA
✅ **RFC 1035 zone file format** support
✅ **YAML configuration** for easy setup
✅ **Structured logging** with tracing
✅ **Async I/O** using tokio for high concurrency
✅ **Proper DNS error responses**: NXDOMAIN, REFUSED, FORMERR, NOERROR

## Installation

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))

### Building from Source

```bash
git clone <repository>
cd lrmdns
cargo build --release
```

The binary will be available at `target/release/lrmdns`.

## Quick Start

1. Create a configuration file (`lrmdns.yaml`):

```yaml
server:
  listen: "127.0.0.1:5353"
  workers: 4
  log_level: info

zones:
  - name: example.com.
    file: zones/example.com.zone
```

2. Create a zone file (`zones/example.com.zone`):

```
$ORIGIN example.com.
$TTL 3600

@       IN  SOA   ns1.example.com. admin.example.com. (
                  2025120601  ; Serial
                  7200        ; Refresh
                  3600        ; Retry
                  1209600     ; Expire
                  86400 )     ; Minimum TTL

@       IN  NS    ns1.example.com.
@       IN  NS    ns2.example.com.

@       IN  A     192.0.2.1
www     IN  A     192.0.2.2
ns1     IN  A     192.0.2.10
ns2     IN  A     192.0.2.11
```

3. Run the server:

```bash
cargo run -- lrmdns.yaml
```

Or with the release build:

```bash
./target/release/lrmdns lrmdns.yaml
```

## Testing

Run unit tests:

```bash
cargo test
```

Test with dig (in another terminal):

```bash
# Query an A record
dig @127.0.0.1 -p 5353 www.example.com A

# Query NS records
dig @127.0.0.1 -p 5353 example.com NS

# Query non-existent name (should return NXDOMAIN)
dig @127.0.0.1 -p 5353 nonexistent.example.com A
```

## Configuration

### Server Configuration

- `listen`: IP address and port to bind (default: `0.0.0.0:53`)
- `workers`: Number of worker threads (default: `4`)
- `log_level`: Logging level - `trace`, `debug`, `info`, `warn`, `error` (default: `info`)

### Zone Configuration

- `name`: Fully qualified domain name (must end with `.`)
- `file`: Path to the zone file

## Zone File Format

lrmdns supports standard RFC 1035 zone file format:

- **Directives**: `$ORIGIN`, `$TTL`
- **Record types**: SOA, NS, A, AAAA
- **Comments**: Lines starting with `;`
- **@ symbol**: Represents the zone origin
- **Relative names**: Automatically appended with zone origin

### Required Records

Each zone file MUST contain:
- One SOA (Start of Authority) record
- At least one NS (Name Server) record

## Architecture

```
src/
├── main.rs       # Entry point, configuration loading
├── config.rs     # Configuration structures and parsing
├── zone.rs       # Zone data structures and zone file parser
├── protocol.rs   # DNS query processing logic
└── server.rs     # UDP server implementation
```

### Key Components

1. **ZoneStore**: In-memory hash map for fast zone lookups
2. **QueryProcessor**: Handles DNS query logic and response building
3. **DnsServer**: Async UDP server using tokio
4. **Zone Parser**: RFC 1035 zone file parser

## Logging

lrmdns uses structured logging via the `tracing` crate. Set the log level in the configuration file or via environment variable:

```bash
RUST_LOG=lrmdns=debug cargo run -- lrmdns.yaml
```

## Performance

Phase 1 focuses on correctness and simplicity. Performance optimizations will come in later phases.

Expected performance:
- **Latency**: <1ms for local queries
- **Throughput**: Thousands of queries per second on modern hardware
- **Concurrency**: Handles multiple concurrent queries via tokio

## Limitations (Phase 1)

- **UDP only** (no TCP support yet)
- **No EDNS0** support (512 byte limit for responses)
- **Limited record types** (A, AAAA, NS, SOA only)
- **No CNAME**, MX, TXT support yet
- **No zone transfers** (AXFR)
- **No dynamic updates**
- **No DNSSEC**
- **No zone reloading** without restart

These features are planned for future phases (see `plan.md`).

## Roadmap

See `plan.md` for the full implementation roadmap:

- **Phase 1** (MVP): ✅ Complete
- **Phase 2** (Core): TCP, CNAME, MX, TXT, EDNS0
- **Phase 3** (Production): Zone reloading, metrics, rate limiting
- **Phase 4** (Advanced): Wildcards, AXFR, additional record types
- **Phase 5** (Future): DNSSEC

## Contributing

This is a learning/experimental project. Contributions welcome!

## License

MIT (or your preferred license)

## Resources

- [RFC 1035 - DNS Specification](https://www.rfc-editor.org/rfc/rfc1035)
- [hickory-dns](https://github.com/hickory-dns/hickory-dns) - DNS library used for protocol handling
- [tokio](https://tokio.rs/) - Async runtime
