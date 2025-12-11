#!/usr/bin/env bash
# Validate test prerequisites

set -e

echo "Validating integration test setup..."

# Check BATS
if [ ! -f "bats/bats-core/bin/bats" ]; then
    echo "Error: BATS not found. Run: git submodule update --init --recursive"
    exit 1
fi

# Check dig
if ! command -v dig &>/dev/null; then
    echo "Error: dig not found. Install bind-tools or dnsutils"
    exit 1
fi

# Check nc (netcat)
if ! command -v nc &>/dev/null; then
    echo "Error: nc (netcat) not found. Install netcat package"
    exit 1
fi

# Check tcpreplay (optional)
if ! command -v tcpreplay &>/dev/null; then
    echo "Info: tcpreplay not found (optional for PCAP tests)"
fi

# Check python3 (for PCAP generation)
if ! command -v python3 &>/dev/null; then
    echo "Warning: python3 not found (needed for PCAP generation)"
fi

# Check scapy (for PCAP)
if ! python3 -c "import scapy" 2>/dev/null; then
    echo "Info: scapy not found (optional for PCAP generation)"
    echo "      Install with: pip3 install scapy"
fi

# Check lrmdns binary
if [ ! -f "../target/release/lrmdns" ]; then
    echo "Warning: lrmdns binary not found. Building..."
    (cd .. && cargo build --release)
fi

echo "Setup validation complete!"
