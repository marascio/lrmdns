```
██╗     ██████╗ ███╗   ███╗██████╗ ███╗   ██╗███████╗
██║     ██╔══██╗████╗ ████║██╔══██╗████╗  ██║██╔════╝
██║     ██████╔╝██╔████╔██║██║  ██║██╔██╗ ██║███████╗
██║     ██╔══██╗██║╚██╔╝██║██║  ██║██║╚██╗██║╚════██║
███████╗██║  ██║██║ ╚═╝ ██║██████╔╝██║ ╚████║███████║
╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝╚═════╝ ╚═╝  ╚═══╝╚══════╝
```

# lrmdns - Lightweight Authoritative DNS Server

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

A simple, fast, and reliable authoritative-only DNS server written in Rust.

## Overview

lrmdns is an authoritative DNS server that responds to queries for domains it manages. It does NOT perform recursive resolution or caching - it only serves zones that are explicitly configured.

## Features

### Phase 1 (MVP) - ✅ Complete
- **Authoritative DNS responses** for configured zones
- **UDP server** on configurable port
- **Standard DNS record types**: A, AAAA, NS, SOA
- **RFC 1035 zone file format** support
- **YAML configuration** for easy setup
- **Structured logging** with tracing
- **Async I/O** using tokio for high concurrency
- **Proper DNS error responses**: NXDOMAIN, REFUSED, FORMERR, NOERROR

### Phase 2 (Core Features) - ✅ Complete
- **TCP server** support for larger responses and zone transfers
- **Additional record types**: CNAME, MX, TXT
- **CNAME chain resolution** - automatically chases CNAMEs to final targets
- **EDNS0 support** - handles larger UDP responses (up to 4096 bytes)
- **Enhanced response completeness** - proper authority and additional sections

### Phase 5 (DNSSEC) - ✅ Complete
- **DNSSEC record types**: DNSKEY, RRSIG, NSEC, DS
- **EDNS0 DNSSEC OK (DO) flag** - proper DNSSEC-aware responses
- **Offline signing support** - serve pre-signed zones
- **Zone file parsing** for DNSSEC records with base64/hex encoding

## Installation

### Prerequisites

- Rust 1.88.0+ (install via [rustup](https://rustup.rs/))
  - Required for Rust 2024 edition features

### Building from Source

```bash
git clone https://github.com/marascio/lrmdns.git
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
- **Record types**: SOA, NS, A, AAAA, CNAME, MX, TXT, PTR, SRV, CAA, DNSKEY, RRSIG, NSEC, DS
- **Comments**: Lines starting with `;`
- **@ symbol**: Represents the zone origin
- **Relative names**: Automatically appended with zone origin
- **Wildcards**: `*` for wildcard matching

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
└── server.rs     # UDP and TCP server implementation
```

### Key Components

1. **ZoneStore**: In-memory hash map for fast zone lookups
2. **QueryProcessor**: Handles DNS query logic, response building, and CNAME resolution
3. **DnsServer**: Async UDP and TCP server using tokio
4. **Zone Parser**: RFC 1035 zone file parser supporting multiple record types

## Logging

lrmdns uses structured logging via the `tracing` crate. Set the log level in the configuration file or via environment variable:

```bash
RUST_LOG=lrmdns=debug cargo run -- lrmdns.yaml
```

## Performance

Phases 1 and 2 focus on correctness and feature completeness. Performance optimizations will come in later phases.

Expected performance:
- **Latency**: <1ms for local queries (UDP), <2ms (TCP)
- **Throughput**: Thousands of queries per second on modern hardware
- **Concurrency**: Handles multiple concurrent connections via tokio
- **Protocol**: UDP for speed, TCP for reliability and larger responses

## DNSSEC Support

lrmdns provides comprehensive DNSSEC support for serving pre-signed zones with validation capabilities.

### Supported DNSSEC Record Types

- **DNSKEY**: Public key distribution
- **RRSIG**: Resource record signatures
- **NSEC**: Authenticated denial of existence
- **DS**: Delegation signer records

### DNSSEC Capabilities

#### Automatic RRSIG Inclusion
When clients set the DNSSEC OK (DO) flag in EDNS0, lrmdns automatically includes RRSIG records with responses:

```bash
# Query with DNSSEC OK flag - RRSIG records automatically included
dig @127.0.0.1 -p 15353 +dnssec example.com A
```

#### Validation Functions
The dnssec module provides:
- **DS digest validation**: Verify DS records against DNSKEY records (SHA-256, SHA-384, SHA-512)
- **RRSIG time validity checking**: Verify signatures are within inception/expiration window
- **NSEC denial validation**: Validate authenticated denial of existence
- **Key tag computation**: RFC 4034 compliant key tag calculation

#### Configuration
DNSSEC behavior can be configured in `lrmdns.yaml`:

```yaml
server:
  dnssec:
    validate_signatures: false      # Signature verification (future)
    require_dnssec: false            # Require DNSSEC for all responses
    auto_include_dnssec: true        # Include RRSIG with DO flag (default)
```

### Zone File Format

DNSSEC records use standard zone file format with base64 and hex encoding:

```
; DNSKEY: flags protocol algorithm public_key_base64
@ IN DNSKEY 256 3 8 AwEAAaetidLzsKWUt4swWR8yu0wPHPiUi8LU...

; RRSIG: type_covered algorithm labels original_ttl sig_expiration sig_inception key_tag signer signature_base64
@ IN RRSIG A 8 2 3600 1767139200 1764547200 12345 example.com. AwEAAaetidLzsKWU...

; NSEC: next_domain_name type_bit_maps...
@ IN NSEC www.example.com. A NS SOA RRSIG NSEC DNSKEY

; DS: key_tag algorithm digest_type digest_hex
@ IN DS 12345 8 2 A8B1C2D3E4F506172839405A6B7C8D9E0F1A2B3C4D5E6F70
```

### Testing DNSSEC Queries

Query DNSSEC records with dig using the `+dnssec` flag:

```bash
# Query with DNSSEC OK flag set - automatically includes RRSIGs
dig @127.0.0.1 -p 15353 +dnssec example.com A

# Query for specific DNSSEC record types
dig @127.0.0.1 -p 15353 example.com DNSKEY
dig @127.0.0.1 -p 15353 example.com DS
dig @127.0.0.1 -p 15353 example.com NSEC
```

### Signing Your Zones

lrmdns does NOT perform online signing. You must sign zones offline using tools like:
- **ldns-signzone** (from ldns)
- **dnssec-signzone** (from BIND)

Once signed, point your zone configuration to the signed zone file.

### Validation Implementation Status

- ✅ **DS digest validation**: Fully implemented with SHA-256/384/512 support
- ✅ **RRSIG time validity**: Checks inception/expiration timestamps
- ✅ **NSEC denial validation**: Validates authenticated denial of existence
- ✅ **Key tag computation**: RFC 4034 compliant algorithm
- ✅ **Automatic DNSSEC record inclusion**: RRSIG records with DO flag
- ⚠️ **Cryptographic signature verification**: Framework ready, full verification not yet implemented
- ❌ **NSEC3 support**: Not implemented (only NSEC for authenticated denial)

## Current Limitations

- **No online DNSSEC signing** - zones must be pre-signed offline
- **No NSEC3 support** - only NSEC for authenticated denial
- **No dynamic updates** (RFC 2136)

See `plan.md` for the full implementation roadmap.

## Roadmap

- **Phase 1** (MVP): ✅ Complete - Basic authoritative DNS with UDP
- **Phase 2** (Core): ✅ Complete - TCP, CNAME, MX, TXT, EDNS0
- **Phase 3** (Production): ✅ Complete - Zone reloading, metrics, rate limiting, privilege management
- **Phase 4** (Advanced): ✅ Complete - Wildcards, AXFR, PTR/SRV/CAA records, management API
- **Phase 5** (DNSSEC): ✅ Complete - Offline DNSSEC support with DNSKEY, RRSIG, NSEC, DS

## Contributing

This is a learning/experimental project. Contributions welcome!

## License

MIT

## Resources

- [RFC 1035 - DNS Specification](https://www.rfc-editor.org/rfc/rfc1035)
- [hickory-dns](https://github.com/hickory-dns/hickory-dns) - DNS library used for protocol handling
- [tokio](https://tokio.rs/) - Async runtime
