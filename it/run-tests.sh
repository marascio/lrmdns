#!/usr/bin/env bash
# Run all integration tests

set -e

cd "$(dirname "$0")"

# Parse arguments
# Default: run in parallel (auto-detect CPUs)
PARALLEL_JOBS=""
RUN_SERIAL=false
BATS_ARGS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        -j|--jobs)
            PARALLEL_JOBS="$2"
            RUN_SERIAL=false
            shift 2
            ;;
        -s|--serial)
            RUN_SERIAL=true
            shift
            ;;
        *)
            BATS_ARGS+=("$1")
            shift
            ;;
    esac
done

# Auto-detect number of CPUs if not set and not running serially
if [ "$RUN_SERIAL" = false ] && [ -z "$PARALLEL_JOBS" ]; then
    if command -v nproc &>/dev/null; then
        PARALLEL_JOBS=$(nproc)
    elif command -v sysctl &>/dev/null; then
        PARALLEL_JOBS=$(sysctl -n hw.ncpu)
    else
        PARALLEL_JOBS=4
    fi
fi

# Check if GNU parallel or rush is available (required for BATS --jobs flag)
# BATS supports either GNU parallel or shenwei356/rush for parallel execution
# If neither is available, fall back to serial execution
if [ "$RUN_SERIAL" = false ] && ! command -v parallel &>/dev/null && ! command -v rush &>/dev/null; then
    echo "Warning: Neither GNU parallel nor rush found. Falling back to serial execution."
    echo "  To enable parallel testing, install either:"
    echo "    - GNU parallel: Ubuntu/Debian: sudo apt-get install parallel, macOS: brew install parallel"
    echo "    - rush: cargo install rush-cli"
    RUN_SERIAL=true
fi

# Check prerequisites
./scripts/validate-setup.sh

# Build lrmdns if needed
if [ ! -f "../target/release/lrmdns" ]; then
    echo "Building lrmdns..."
    (cd .. && cargo build --release)
fi

# Run BATS tests
if [ "$RUN_SERIAL" = true ]; then
    echo "Running integration tests..."
    bats/bats-core/bin/bats tests/*.bats "${BATS_ARGS[@]}"
else
    echo "Running integration tests in parallel (${PARALLEL_JOBS} jobs)..."
    bats/bats-core/bin/bats --jobs "$PARALLEL_JOBS" tests/*.bats "${BATS_ARGS[@]}"
fi
