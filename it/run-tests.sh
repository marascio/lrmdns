#!/usr/bin/env bash
# Run all integration tests

set -e

cd "$(dirname "$0")"

# Parse arguments
PARALLEL_JOBS=""
BATS_ARGS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        -j|--jobs)
            PARALLEL_JOBS="$2"
            shift 2
            ;;
        --parallel)
            # Auto-detect number of CPUs
            if command -v nproc &>/dev/null; then
                PARALLEL_JOBS=$(nproc)
            elif command -v sysctl &>/dev/null; then
                PARALLEL_JOBS=$(sysctl -n hw.ncpu)
            else
                PARALLEL_JOBS=4
            fi
            shift
            ;;
        *)
            BATS_ARGS+=("$1")
            shift
            ;;
    esac
done

# Check prerequisites
./scripts/validate-setup.sh

# Build lrmdns if needed
if [ ! -f "../target/release/lrmdns" ]; then
    echo "Building lrmdns..."
    (cd .. && cargo build --release)
fi

# Run BATS tests
if [ -n "$PARALLEL_JOBS" ]; then
    echo "Running integration tests in parallel (${PARALLEL_JOBS} jobs)..."
    bats/bats-core/bin/bats --jobs "$PARALLEL_JOBS" tests/*.bats "${BATS_ARGS[@]}"
else
    echo "Running integration tests..."
    bats/bats-core/bin/bats tests/*.bats "${BATS_ARGS[@]}"
fi
