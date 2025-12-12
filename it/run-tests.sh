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
  -p, --profile PROFILE   Build profile: release or debug (default: release)
  -c, --coverage          Generate code coverage report (requires cargo-llvm-cov)

BATS_ARGS:
  Additional arguments passed directly to BATS (e.g., test file patterns)

EXAMPLES:
  run-tests.sh                           Run all tests in parallel
  run-tests.sh --serial                  Run all tests serially
  run-tests.sh --jobs 4                  Run with 4 parallel jobs
  run-tests.sh --timeout 120             Set 120 second timeout
  run-tests.sh --profile debug           Run tests with debug binary
  run-tests.sh --coverage                Generate coverage report
  run-tests.sh tests/01-basic-queries.bats  Run specific test file

ENVIRONMENT:
  BATS_TEST_TIMEOUT       Test timeout (set via --timeout or defaults to 60)
  LRMDNS_BIN              Path to lrmdns binary (overrides --profile)

COVERAGE:
  Coverage requires cargo-llvm-cov: cargo install cargo-llvm-cov
  Output: coverage.lcov (LCOV format for Codecov)

EOF
}

# Parse arguments
# Default: run in parallel (auto-detect CPUs), 60 second timeout, release profile, no coverage
PARALLEL_JOBS=""
RUN_SERIAL=false
BUILD_PROFILE="release"
ENABLE_COVERAGE=false
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
        -c|--coverage)
            ENABLE_COVERAGE=true
            shift
            ;;
        *)
            BATS_ARGS+=("$1")
            shift
            ;;
    esac
done

# Set LRMDNS_BIN based on coverage and profile
if [ "$LRMDNS_BIN_SOURCE" = "profile" ]; then
    if [ "$ENABLE_COVERAGE" = true ]; then
        # cargo-llvm-cov puts instrumented binaries in a different location
        export LRMDNS_BIN="../target/llvm-cov-target/${BUILD_PROFILE}/lrmdns"
    else
        export LRMDNS_BIN="../target/${BUILD_PROFILE}/lrmdns"
    fi
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
    echo "  To enable parallel testing, install either:"
    echo "    - GNU parallel: Ubuntu/Debian: sudo apt-get install parallel, macOS: brew install parallel"
    echo "    - rush: cargo install rush-cli"
    RUN_SERIAL=true
fi

# Print test configuration summary
echo "Integration Test Configuration"
echo "------------------------------"
printf "Platform : %s\n" "${OSTYPE}"
if [ "$RUN_SERIAL" = true ] || [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
    printf "    Mode : %s\n" "Serial"
    printf "    Jobs : %s\n" "1"
else
    printf "    Mode : %s\n" "Parallel"
    printf "    Jobs : %s\n" "${PARALLEL_JOBS}"
fi
printf " Timeout : %ss\n" "${BATS_TEST_TIMEOUT}"
if [ "$LRMDNS_BIN_SOURCE" = "environment" ]; then
    printf "  Binary : %s (from environment)\n" "${LRMDNS_BIN}"
else
    if [ "$ENABLE_COVERAGE" = true ]; then
        printf "  Binary : %s (%s, coverage)\n" "${LRMDNS_BIN}" "${BUILD_PROFILE}"
    else
        printf "  Binary : %s (%s)\n" "${LRMDNS_BIN}" "${BUILD_PROFILE}"
    fi
fi
echo

# Check prerequisites
./scripts/validate-setup.sh

# Build lrmdns if needed (only when using profile-based path)
if [ "$LRMDNS_BIN_SOURCE" = "profile" ] && [ ! -f "$LRMDNS_BIN" ]; then
    if [ "$ENABLE_COVERAGE" = true ]; then
        echo "Building lrmdns with coverage instrumentation (${BUILD_PROFILE})..."
        # Use 'cargo llvm-cov run' to build an instrumented binary
        # The --bin flag ensures we build the binary, not just run tests
        # Need to run from project root where Cargo.toml is
        if [ "$BUILD_PROFILE" = "release" ]; then
            (cd .. && cargo llvm-cov run --bin lrmdns --release --no-report -- --help) > /dev/null 2>&1
        else
            (cd .. && cargo llvm-cov run --bin lrmdns --no-report -- --help) > /dev/null 2>&1
        fi
    else
        echo "Building lrmdns (${BUILD_PROFILE})..."
        if [ "$BUILD_PROFILE" = "release" ]; then
            (cd .. && cargo build --release)
        else
            (cd .. && cargo build)
        fi
    fi
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

# Generate coverage report if coverage was enabled
if [ "$ENABLE_COVERAGE" = true ]; then
    echo
    echo "Generating coverage report..."
    # Need to run from project root where Cargo.toml is
    (cd .. && cargo llvm-cov report --lcov --output-path it/coverage.lcov)
    echo "Coverage report written to: it/coverage.lcov"
fi
