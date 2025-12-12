#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

# Tests for zone reloading via SIGHUP signal (Unix only)

setup() {
    # Skip on Windows - SIGHUP is not supported
    if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
        skip "SIGHUP signal not supported on Windows"
    fi

    # Create test-specific files in BATS_TEST_TMPDIR for automatic isolation and cleanup
    TEST_ZONE="$BATS_TEST_TMPDIR/example.zone"
    TEST_CONFIG="$BATS_TEST_TMPDIR/config.yaml"
    # BATS_TEST_TMPDIR is fresh for each test - no cleanup needed

    # Get unique port for this test
    local port=$(get_unique_port)

    # Create initial zone file
    cat > "$TEST_ZONE" <<'EOF'
$ORIGIN example.com.
$TTL 3600

@ IN SOA (
    ns1.example.com.
    admin.example.com.
    2024010101  ; Serial
    7200        ; Refresh
    3600        ; Retry
    1209600     ; Expire
    86400       ; Minimum TTL
)

@ IN NS ns1.example.com.
@ IN NS ns2.example.com.

ns1 IN A 192.0.2.1
ns2 IN A 192.0.2.2

www IN A 192.0.2.10
mail IN A 192.0.2.20
EOF

    # Create config file
    cat > "$TEST_CONFIG" <<EOF
server:
  listen: "127.0.0.1:${port}"
  workers: 2
  log_level: info

zones:
  - name: example.com.
    file: $TEST_ZONE
EOF

    # Start server with the test config
    start_server "$TEST_CONFIG" "$port"
}

teardown() {
    cleanup_server
    # BATS automatically cleans up BATS_TEST_TMPDIR - no manual file deletion needed
}

@test "Zone reload via SIGHUP - modify existing A record" {
    if is_windows; then
        skip "SIGHUP signal not supported on Windows"
    fi

    # Query initial value
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"

    # Modify zone file
    sed -i.bak 's/www IN A 192.0.2.10/www IN A 192.0.2.99/' "$TEST_ZONE"

    # Send SIGHUP to reload zones
    reload_server

    # Query should return new value
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.99"
}

@test "Zone reload via SIGHUP - add new A record" {
    if is_windows; then
        skip "SIGHUP signal not supported on Windows"
    fi

    # Query non-existent record (should be NXDOMAIN)
    local rcode=$(get_rcode "new.example.com." A)
    assert_equal "$rcode" "NXDOMAIN"

    # Add new record to zone file
    echo "new IN A 192.0.2.30" >> "$TEST_ZONE"

    # Reload zones
    reload_server

    # Query should now return the new record
    result=$(query_a "new.example.com.")
    assert_equal "$result" "192.0.2.30"
}

@test "Zone reload via SIGHUP - delete A record" {
    if is_windows; then
        skip "SIGHUP signal not supported on Windows"
    fi

    # Query should succeed initially
    result=$(query_a "mail.example.com.")
    assert_equal "$result" "192.0.2.20"

    # Remove the record from zone file
    sed -i.bak '/mail IN A/d' "$TEST_ZONE"

    # Reload zones
    reload_server

    # Query should now return NXDOMAIN
    local rcode=$(get_rcode "mail.example.com." A)
    assert_equal "$rcode" "NXDOMAIN"
}

@test "Zone reload via SIGHUP - update SOA serial" {
    if is_windows; then
        skip "SIGHUP signal not supported on Windows"
    fi

    # Query initial SOA
    run dig @127.0.0.1 -p "$LRMDNS_PORT" example.com. SOA +short
    assert_success
    assert_output --partial "2024010101"

    # Update SOA serial
    sed -i.bak 's/2024010101/2024010102/' "$TEST_ZONE"

    # Reload zones
    reload_server

    # Query should return new serial
    run dig @127.0.0.1 -p "$LRMDNS_PORT" example.com. SOA +short
    assert_success
    assert_output --partial "2024010102"
}

@test "Zone reload via SIGHUP - add multiple record types" {
    if is_windows; then
        skip "SIGHUP signal not supported on Windows"
    fi

    # Add MX, TXT, and CNAME records
    cat >> "$TEST_ZONE" <<'EOF'
@ IN MX 10 mail.example.com.
@ IN TXT "v=spf1 mx -all"
ftp IN CNAME www.example.com.
EOF

    # Reload zones
    reload_server

    # Verify MX record
    run dig @127.0.0.1 -p "$LRMDNS_PORT" example.com. MX +short
    assert_success
    assert_output --partial "mail.example.com"

    # Verify TXT record
    run dig @127.0.0.1 -p "$LRMDNS_PORT" example.com. TXT +short
    assert_success
    assert_output --partial "v=spf1 mx -all"

    # Verify CNAME (should resolve to www's IP)
    result=$(query_a "ftp.example.com." | tail -1)
    assert_equal "$result" "192.0.2.10"
}

@test "Zone reload via SIGHUP - invalid zone file preserves old zone" {
    if is_windows; then
        skip "SIGHUP signal not supported on Windows"
    fi

    # Initial query should work
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"

    # Corrupt the zone file (remove SOA record)
    sed -i.bak '/@ IN SOA/,/)/d' "$TEST_ZONE"

    # Try to reload (should fail but not crash)
    kill -HUP "$LRMDNS_PID" || true
    sleep 0.5

    # Server should still serve the old zone
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"

    # Check server is still running
    kill -0 "$LRMDNS_PID"
}

@test "Zone reload via SIGHUP - multiple reloads in succession" {
    if is_windows; then
        skip "SIGHUP signal not supported on Windows"
    fi

    # First reload - change to 192.0.2.50
    sed -i.bak 's/www IN A 192.0.2.10/www IN A 192.0.2.50/' "$TEST_ZONE"
    reload_server
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.50"

    # Second reload - change to 192.0.2.60
    sed -i.bak 's/www IN A 192.0.2.50/www IN A 192.0.2.60/' "$TEST_ZONE"
    reload_server
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.60"

    # Third reload - change to 192.0.2.70
    sed -i.bak 's/www IN A 192.0.2.60/www IN A 192.0.2.70/' "$TEST_ZONE"
    reload_server
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.70"
}

@test "Zone reload via SIGHUP - change multiple zones" {
    if is_windows; then
        skip "SIGHUP signal not supported on Windows"
    fi

    # Create second zone in same test temp directory
    local zone2="$BATS_TEST_TMPDIR/test.zone"
    cat > "$zone2" <<'EOF'
$ORIGIN test.com.
$TTL 3600

@ IN SOA (
    ns1.test.com.
    admin.test.com.
    2024010101
    7200
    3600
    1209600
    86400
)

@ IN NS ns1.test.com.
ns1 IN A 192.0.2.100
www IN A 192.0.2.110
EOF

    # Update config to include both zones
    cat > "$TEST_CONFIG" <<EOF
server:
  listen: "127.0.0.1:${LRMDNS_PORT}"
  workers: 2
  log_level: info

zones:
  - name: example.com.
    file: $TEST_ZONE
  - name: test.com.
    file: $zone2
EOF

    # Restart server to pick up new zone (config changes require restart)
    cleanup_server
    start_server "$TEST_CONFIG" "$LRMDNS_PORT"

    # Verify both zones work
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"

    result=$(query_a "www.test.com.")
    assert_equal "$result" "192.0.2.110"

    # Modify both zone files
    sed -i.bak 's/www IN A 192.0.2.10/www IN A 192.0.2.11/' "$TEST_ZONE"
    sed -i.bak 's/www IN A 192.0.2.110/www IN A 192.0.2.111/' "$zone2"

    # Reload
    reload_server

    # Verify both zones updated
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.11"

    result=$(query_a "www.test.com.")
    assert_equal "$result" "192.0.2.111"
    # BATS automatically cleans up zone2 in BATS_TEST_TMPDIR
}
