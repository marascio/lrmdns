#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

# Tests for various SOA record format support

@test "Single-line SOA format" {
    # Get unique port and temp file for this test
    local port=$(get_unique_port)
    local config="$(get_temp_prefix)-soa-single.yaml"

    # Create temporary config for this zone
    cat > "$config" <<EOF
server:
  listen: "127.0.0.1:${port}"
zones:
  - name: test.example.
    file: fixtures/zones/soa-single-line.zone
EOF

    start_server "$config" "$port"

    # Query SOA record
    run dig @127.0.0.1 -p "$port" test.example. SOA +short
    assert_success
    assert_output --partial "ns1.test.example"
    assert_output --partial "hostmaster.test.example"
    assert_output --partial "2024120601"

    # Query A record to ensure zone works
    result=$(dig @127.0.0.1 -p "$port" www.test.example. A +short)
    assert_equal "$result" "192.0.2.100"

    cleanup_server
}

@test "Multi-line SOA with detailed comments" {
    local port=$(get_unique_port)
    local config="$(get_temp_prefix)-soa-comments.yaml"

    cat > "$config" <<EOF
server:
  listen: "127.0.0.1:${port}"
zones:
  - name: test2.example.
    file: fixtures/zones/soa-multiline-comments.zone
EOF

    start_server "$config" "$port"

    # Query SOA record
    run dig @127.0.0.1 -p "$port" test2.example. SOA +short
    assert_success
    assert_output --partial "ns1.test2.example"
    assert_output --partial "admin.test2.example"
    assert_output --partial "2024120602"
    assert_output --partial "7200"
    assert_output --partial "3600"

    # Query MX record
    run dig @127.0.0.1 -p "$port" test2.example. MX +short
    assert_success
    assert_output --partial "mail.test2.example"

    cleanup_server
}

@test "Multi-line SOA compact (no comments)" {
    local port=$(get_unique_port)
    local config="$(get_temp_prefix)-soa-compact.yaml"

    cat > "$config" <<EOF
server:
  listen: "127.0.0.1:${port}"
zones:
  - name: test3.example.
    file: fixtures/zones/soa-multiline-compact.zone
EOF

    start_server "$config" "$port"

    # Query SOA record
    run dig @127.0.0.1 -p "$port" test3.example. SOA +short
    assert_success
    assert_output --partial "2024120603"
    assert_output --partial "14400"

    # Query NS record
    run dig @127.0.0.1 -p "$port" test3.example. NS +short
    assert_success
    assert_output --partial "ns1.test3.example"

    cleanup_server
}

@test "Multi-line SOA with mixed grouping" {
    local port=$(get_unique_port)
    local config="$(get_temp_prefix)-soa-mixed.yaml"

    cat > "$config" <<EOF
server:
  listen: "127.0.0.1:${port}"
zones:
  - name: test4.example.
    file: fixtures/zones/soa-multiline-mixed.zone
EOF

    start_server "$config" "$port"

    # Query SOA record
    run dig @127.0.0.1 -p "$port" test4.example. SOA +short
    assert_success
    assert_output --partial "2024120604"
    assert_output --partial "10800"
    assert_output --partial "604800"

    # Query CNAME record
    run dig @127.0.0.1 -p "$port" ftp.test4.example. A +short
    assert_success
    assert_output --partial "192.0.2.90"

    cleanup_server
}

@test "Multi-line SOA with tabs and mixed whitespace" {
    local port=$(get_unique_port)
    local config="$(get_temp_prefix)-soa-tabs.yaml"

    cat > "$config" <<EOF
server:
  listen: "127.0.0.1:${port}"
zones:
  - name: test5.example.
    file: fixtures/zones/soa-tabs-and-spaces.zone
EOF

    start_server "$config" "$port"

    # Query SOA record
    run dig @127.0.0.1 -p "$port" test5.example. SOA +short
    assert_success
    assert_output --partial "2024120605"
    assert_output --partial "14400"
    assert_output --partial "7200"

    # Query A record
    result=$(dig @127.0.0.1 -p "$port" ns1.test5.example. A +short)
    assert_equal "$result" "192.0.2.91"

    cleanup_server
}

@test "Standard multi-line SOA (basic.zone)" {
    local port=$(get_unique_port)
    local config="$(get_temp_prefix)-basic.yaml"

    # This tests the standard format we use in basic.zone
    cat > "$config" <<EOF
server:
  listen: "127.0.0.1:${port}"
zones:
  - name: example.com.
    file: fixtures/zones/basic.zone
EOF

    start_server "$config" "$port"

    # Query SOA record
    run dig @127.0.0.1 -p "$port" example.com. SOA +short
    assert_success
    assert_output --partial "ns1.example.com"
    assert_output --partial "admin.example.com"
    assert_output --partial "2024010101"
    assert_output --partial "7200"
    assert_output --partial "86400"

    cleanup_server
}

@test "All SOA formats return valid authoritative responses" {
    local port=$(get_unique_port)
    local config="$(get_temp_prefix)-multi-zone.yaml"

    # Test that all formats produce authoritative responses
    cat > "$config" <<EOF
server:
  listen: "127.0.0.1:${port}"
zones:
  - name: test.example.
    file: fixtures/zones/soa-single-line.zone
  - name: test2.example.
    file: fixtures/zones/soa-multiline-comments.zone
  - name: test3.example.
    file: fixtures/zones/soa-multiline-compact.zone
EOF

    start_server "$config" "$port"

    # Check each zone has authoritative flag
    for zone in test.example test2.example test3.example; do
        run dig @127.0.0.1 -p "$port" $zone SOA +noall +comments
        assert_success
        assert_output --regexp "flags:.*aa"
    done

    cleanup_server
}
