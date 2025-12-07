#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

# Tests for TCP connection pooling and timeout features

setup() {
    start_server fixtures/configs/basic.yaml
}

teardown() {
    cleanup_server
}

@test "TCP connection handles multiple queries on same connection" {
    # Use dig with +keepopen to reuse TCP connection - single invocation with multiple queries
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" +tcp +keepopen +short \
        www.example.com. A \
        mail.example.com. A \
        ns1.example.com. A)

    # Should get all three responses
    assert [ -n "$result" ]
    echo "$result" | grep -q "192.0.2.10"
    echo "$result" | grep -q "192.0.2.20"
    echo "$result" | grep -q "192.0.2.1"
}

@test "TCP connection reuse - five queries on same connection" {
    # Make 5 queries on the same TCP connection using +keepopen
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" +tcp +keepopen +short \
        www.example.com. A \
        www.example.com. A \
        www.example.com. A \
        www.example.com. A \
        www.example.com. A)

    # Should get 5 responses (all the same IP)
    count=$(echo "$result" | grep -c "192.0.2.10")
    assert_equal "$count" "5"
}

@test "TCP connection with different record types using keepopen" {
    # Query different types on single connection
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" +tcp +keepopen +short \
        www.example.com. A \
        www.example.com. AAAA \
        example.com. MX)

    # Should get responses for all three
    echo "$result" | grep -q "192.0.2.10"
    echo "$result" | grep -q "2001:db8::10"
    echo "$result" | grep -q "mail.example.com"
}

@test "TCP connection with NXDOMAIN query on same connection" {
    # Query valid, then NXDOMAIN, then valid again on same connection
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" +tcp +keepopen \
        www.example.com. A \
        nonexistent.example.com. A \
        mail.example.com. A)

    # Should get valid responses for first and third queries
    echo "$result" | grep -q "192.0.2.10"
    echo "$result" | grep -q "192.0.2.20"
    # NXDOMAIN query should appear in output
    echo "$result" | grep -q "NXDOMAIN"
}
