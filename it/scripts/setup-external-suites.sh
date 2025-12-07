#!/usr/bin/env bash
# Download and setup external test suites

set -e

mkdir -p fixtures/external

# Setup Deckard
if [ ! -d "fixtures/external/deckard" ]; then
    echo "Cloning Deckard..."
    git clone https://github.com/CZ-NIC/deckard.git fixtures/external/deckard

    # Install Deckard dependencies
    cd fixtures/external/deckard
    if command -v pip3 &>/dev/null; then
        pip3 install -r requirements.txt
    else
        echo "Warning: pip3 not found. Deckard dependencies not installed."
    fi
    cd -
fi

# Setup FerretDataset
if [ ! -d "fixtures/external/ferret" ]; then
    echo "Cloning FerretDataset..."
    git clone https://github.com/dns-groot/FerretDataset.git fixtures/external/ferret
fi

echo "External test suites setup complete!"
