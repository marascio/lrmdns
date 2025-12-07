# lrmdns Integration Tests

BATS-based integration tests for lrmdns DNS server.

## Overview

This directory contains integration tests that validate lrmdns functionality using external DNS tools and actual network queries. Tests are written in BATS (Bash Automated Testing System) and are completely independent of the Rust codebase.

## Prerequisites

### Required

- **BATS** - Bash Automated Testing System (included as git submodule)
- **dig** - DNS lookup tool from BIND
- **curl** - HTTP client (for API tests)

### Optional

- **tcpreplay** - For PCAP replay tests
- **python3 + scapy** - For PCAP generation
- **jq** - For JSON parsing in API tests
- **GNU parallel** - For parallel test execution (BATS `--jobs` support)

### Installation

#### macOS

```bash
brew install bind curl jq tcpreplay parallel
pip3 install scapy
```

#### Ubuntu/Debian

```bash
apt-get install dnsutils curl jq tcpreplay parallel
pip3 install scapy
```

## Setup

```bash
# Initialize BATS submodules
git submodule update --init --recursive

# Validate setup
cd it
./scripts/validate-setup.sh

# Build lrmdns
cd .. && cargo build --release && cd it
```

## Running Tests

```bash
# Run all tests (sequential)
./run-tests.sh

# Run tests in parallel (auto-detect CPU cores)
./run-tests.sh --parallel

# Run tests in parallel with specific number of jobs
./run-tests.sh -j 4

# Run specific test file
bats/bats-core/bin/bats tests/01-basic-queries.bats

# Run with verbose output
./run-tests.sh --verbose

# Run only tests matching pattern
./run-tests.sh --filter "A record"

# Combine parallel and other options
./run-tests.sh --parallel --verbose
```

### Parallel Execution

Tests are designed to run in parallel safely by:
- Auto-assigning unique ports (20000+) per test based on `BATS_TEST_NUMBER`
- Using test-specific temporary files
- Avoiding shared state between tests

**Note**: Parallel execution requires GNU `parallel` to be installed (see Optional Prerequisites above).

Parallel execution can significantly reduce total test time. Example:
```bash
# Sequential: ~10-15 seconds
./run-tests.sh

# Parallel with 4 jobs: ~3-5 seconds
./run-tests.sh -j 4
```

## Test Organization

Tests are organized numerically by category:

- **00-09**: Core DNS functionality
  - `00-setup.bats` - Prerequisites and setup validation
  - `01-basic-queries.bats` - Basic record queries (A, AAAA, MX, TXT, NS, CNAME)
  - `04-tcp.bats` - TCP protocol support

- **10-19**: Server features
  - `11-metrics.bats` - HTTP API and metrics endpoints

- **20-29**: PCAP-based tests (future)
  - Packet replay and validation tests

- **30-39**: External test suites (future)
  - Deckard and FerretDataset integration

## Directory Structure

```
it/
├── bats/                    # BATS framework (git submodules)
│   ├── bats-core/
│   └── test_helper/
├── helpers/                 # Bash helper functions
│   ├── server               # Server lifecycle management
│   ├── dns                  # DNS query helpers
│   ├── pcap                 # PCAP replay helpers
│   ├── assertions           # Custom assertions
│   └── external             # External suite orchestration
├── fixtures/                # Test data
│   ├── zones/               # Zone files
│   ├── configs/             # Server configurations
│   ├── pcaps/               # PCAP files (future)
│   └── external/            # External test suites (future)
├── tests/                   # Test files
├── scripts/                 # Utility scripts
├── run-tests.sh             # Main test runner
└── README.md                # This file
```

## Writing Tests

### Basic Test Structure

```bash
#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

setup() {
    start_server fixtures/configs/basic.yaml 15353
}

teardown() {
    stop_server
}

@test "Query returns expected result" {
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.1"
}
```

### Available Helper Functions

#### Server Management (`helpers/server`)

- `start_server <config> <port>` - Start lrmdns server
- `stop_server` - Stop running server
- `reload_server` - Send SIGHUP to reload zones
- `get_metrics <api_port>` - Query metrics API

#### DNS Queries (`helpers/dns`)

- `query_a <domain> [port]` - Query A record
- `query_aaaa <domain> [port]` - Query AAAA record
- `query_mx <domain> [port]` - Query MX record
- `query_txt <domain> [port]` - Query TXT record
- `query_ns <domain> [port]` - Query NS record
- `query_tcp <domain> <type> [port]` - Query over TCP
- `query_dnssec <domain> <type> [port]` - Query with DNSSEC
- `query_axfr <zone> [port]` - Zone transfer
- `get_rcode <domain> <type> [port]` - Get response code
- `is_authoritative <domain> [port]` - Check AA flag

#### Assertions (`helpers/assertions`)

- `assert_response_ip <response> <expected_ip>` - Verify IP in response
- `assert_rcode <domain> <expected_rcode> [port]` - Verify response code
- `assert_answer_count <domain> <count> [type] [port]` - Verify answer count

## Test Fixtures

### Zone Files

Zone files are located in `fixtures/zones/`:

- `basic.zone` - Example zone with various record types

### Configuration Files

Server configurations are in `fixtures/configs/`:

- `basic.yaml` - Basic server configuration
- `basic-with-api.yaml` - Configuration with HTTP API enabled

## External Test Suites

The framework supports integration with external DNS test suites:

### Deckard

[Deckard](https://github.com/CZ-NIC/deckard) is a DNS testing framework from CZ-NIC.

```bash
# Setup
./scripts/setup-external-suites.sh

# Run tests
bats/bats-core/bin/bats tests/30-deckard.bats
```

### FerretDataset

[FerretDataset](https://github.com/dns-groot/FerretDataset) is a collection of DNS test cases.

```bash
# Setup
./scripts/setup-external-suites.sh

# Run tests
bats/bats-core/bin/bats tests/31-ferret.bats
```

## Troubleshooting

### Server fails to start

- Check if port 15353 is already in use
- Verify lrmdns binary exists: `../target/release/lrmdns`
- Check server logs: `/tmp/lrmdns-test.log`

### dig command not found

Install BIND tools:
- macOS: `brew install bind`
- Ubuntu: `apt-get install dnsutils`

### BATS not found

Initialize submodules:
```bash
git submodule update --init --recursive
```

## CI/CD Integration

Tests can be run in CI/CD pipelines:

```yaml
# Example GitHub Actions
- name: Run integration tests
  run: |
    cd it
    ./scripts/validate-setup.sh
    ./run-tests.sh
```

## Contributing

When adding new tests:

1. Follow the numeric naming convention
2. Use appropriate helpers from `helpers/`
3. Include both positive and negative test cases
4. Clean up resources in `teardown()`
5. Document any new fixtures or configurations

## License

Same as lrmdns project.
