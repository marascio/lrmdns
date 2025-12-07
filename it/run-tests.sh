#!/usr/bin/env bash
# Run all integration tests

set -e

cd "$(dirname "$0")"

# Check prerequisites
./scripts/validate-setup.sh

# Build lrmdns if needed
if [ ! -f "../target/release/lrmdns" ]; then
    echo "Building lrmdns..."
    (cd .. && cargo build --release)
fi

# Run BATS tests
echo "Running integration tests..."
bats/bats-core/bin/bats tests/*.bats "$@"
