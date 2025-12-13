#!/usr/bin/env bash
# Validate test prerequisites

set -e

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

# Check nc/ncat (netcat) based on platform
# Windows (Git Bash, Cygwin, WSL, MSYS) requires ncat from nmap package
# Unix variants (Linux, macOS) require nc
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
    # Windows - require ncat
    if ! command -v ncat &>/dev/null; then
        echo "Error: ncat not found. On Windows, install nmap package: choco install nmap"
        exit 1
    fi
else
    # Unix (Linux, macOS, BSD, etc.) - require nc
    if ! command -v nc &>/dev/null; then
        echo "Error: nc (netcat) not found. Install netcat package"
        echo "  - Ubuntu/Debian: sudo apt-get install netcat-openbsd"
        echo "  - macOS: brew install netcat"
        exit 1
    fi
fi
