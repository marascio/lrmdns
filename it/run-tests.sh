#!/usr/bin/env bash
# Run all integration tests

set -e

cd "$(dirname "$0")"

# Show help
show_help() {
    cat << EOF
Usage: run-tests.sh [OPTIONS] [BATS_ARGS...]

Run lrmdns integration tests using BATS.

OPTIONS:
  -h, --help              Show this help message and exit
  -j, --jobs N            Run tests in parallel with N jobs (default: auto-detect CPUs)
  -s, --serial            Run tests serially (disables parallelization)
  -t, --timeout SECONDS   Set test timeout in seconds (default: 60)
  -d, --debug             Build profile debug
  -r, --release           Build profile release
  -p, --profile PROFILE   Build profile: release or debug (default: release)

BATS_ARGS:
  Additional arguments passed directly to BATS (e.g., test file patterns)

ENVIRONMENT:
  BATS_TEST_TIMEOUT       Test timeout (set via --timeout or defaults to 60)
  LRMDNS_BIN              Path to lrmdns binary (overrides --profile)

EXAMPLES:
  run-tests.sh                           Run all tests in parallel
  run-tests.sh --serial                  Run all tests serially
  run-tests.sh --jobs 4                  Run with 4 parallel jobs
  run-tests.sh --timeout 120             Set 120 second timeout
  run-tests.sh --profile debug           Run tests with debug binary
  run-tests.sh tests/01-basic-queries.bats  Run specific test file

EOF
}

# Parse arguments
# Default: run in parallel (auto-detect CPUs), 60 second timeout, release profile
PARALLEL_JOBS=""
RUN_SERIAL=false
BUILD_PROFILE="release"
BATS_ARGS=()

# Use existing BATS_TEST_TIMEOUT from environment, or default to 60
: "${BATS_TEST_TIMEOUT:=60}"

# Check if LRMDNS_BIN is already set
if [ -n "$LRMDNS_BIN" ]; then
    LRMDNS_BIN_SOURCE="environment"
else
    LRMDNS_BIN_SOURCE="profile"
fi

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -j|--jobs)
            PARALLEL_JOBS="$2"
            RUN_SERIAL=false
            shift 2
            ;;
        -s|--serial)
            RUN_SERIAL=true
            PARALLEL_JOBS="1"
            shift
            ;;
        -t|--timeout)
            BATS_TEST_TIMEOUT="$2"
            shift 2
            ;;
        -p|--profile)
            BUILD_PROFILE="$2"
            shift 2
            ;;
        -d|--debug)
            BUILD_PROFILE="debug"
            shift
            ;;
        -r|--release)
            BUILD_PROFILE="release"
            shift
            ;;
        *)
            BATS_ARGS+=("$1")
            shift
            ;;
    esac
done

# Set LRMDNS_BIN based on profile
if [ "$LRMDNS_BIN_SOURCE" = "profile" ]; then
    export LRMDNS_BIN="../target/${BUILD_PROFILE}/lrmdns"
fi

# Export BATS_TEST_TIMEOUT for BATS to use
export BATS_TEST_TIMEOUT

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
    echo "         To enable parallel testing, install either GNU parallel or shenwei356/rush"
    RUN_SERIAL=true
    PARALLEL_JOBS="1"
fi

# Print test configuration summary
echo "Integration Test Configuration"
echo "------------------------------"
printf "Platform : %s\n" "${OSTYPE}"
if [ "$RUN_SERIAL" = true ] || [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
    printf "    Mode : %s\n" "Serial"
else
    printf "    Mode : %s\n" "Parallel"
fi
printf "    Jobs : %s\n" "${PARALLEL_JOBS}"
printf " Timeout : %ss\n" "${BATS_TEST_TIMEOUT}"

if [ "$LRMDNS_BIN_SOURCE" = "environment" ]; then
    printf "  Binary : %s (from environment)\n" "${LRMDNS_BIN}"
else
    printf "  Binary : %s (%s)\n" "${LRMDNS_BIN}" "${BUILD_PROFILE}"
fi
echo

# Check prerequisites
./scripts/validate-setup.sh

# Build lrmdns if needed (only when using profile-based path)
if [ "$LRMDNS_BIN_SOURCE" = "profile" ] && [ ! -f "$LRMDNS_BIN" ]; then
    echo "Building lrmdns (${BUILD_PROFILE})..."
    (cd .. && cargo build --${BUILD_PROFILE})
fi

# Run BATS tests
# On Windows (msys/cygwin), always run serially due to tooling limitations
if [ "$RUN_SERIAL" = true ]; then
    bats/bats-core/bin/bats tests/*.bats "${BATS_ARGS[@]}"
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
    bats/bats-core/bin/bats tests/*.bats "${BATS_ARGS[@]}"
else
    bats/bats-core/bin/bats --jobs "$PARALLEL_JOBS" tests/*.bats "${BATS_ARGS[@]}"
fi
