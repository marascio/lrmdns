#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

# Tests for DNS error handling and edge cases

setup() {
    start_server fixtures/configs/basic.yaml
}

teardown() {
    cleanup_server
}

@test "NXDOMAIN for non-existent domain" {
    local rcode=$(get_rcode "nonexistent.example.com." A)
    assert_equal "$rcode" "NXDOMAIN"
}

@test "NXDOMAIN for non-existent subdomain" {
    local rcode=$(get_rcode "does.not.exist.example.com." A)
    assert_equal "$rcode" "NXDOMAIN"
}

@test "NOERROR with empty answer for wrong record type" {
    # Query for A on domain that only has AAAA record (ns1 only has A, so query for AAAA)
    local rcode=$(get_rcode "ns1.example.com." AAAA)
    # Should return NOERROR but no answer
    assert_equal "$rcode" "NOERROR"

    local count=$(dig @127.0.0.1 -p "$LRMDNS_PORT" "ns1.example.com." AAAA +short | wc -l | tr -d ' ')
    assert_equal "$count" "0"
}

@test "REFUSED for query outside configured zones" {
    local rcode=$(get_rcode "www.google.com." A)
    assert_equal "$rcode" "REFUSED"
}

@test "Query for domain in different case" {
    # DNS is case-insensitive
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" WWW.EXAMPLE.COM. A +short)
    assert_equal "$result" "192.0.2.10"

    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" WwW.ExAmPlE.cOm. A +short)
    assert_equal "$result" "192.0.2.10"
}

@test "Query with trailing dot vs without" {
    # With trailing dot
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" www.example.com. A +short)
    assert_equal "$result" "192.0.2.10"

    # Without trailing dot (dig will add it)
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" www.example.com A +short)
    assert_equal "$result" "192.0.2.10"
}

@test "Query for root domain (.)" {
    # Query for root should be refused (we don't serve root)
    local rcode=$(get_rcode "." NS)
    assert_equal "$rcode" "REFUSED"
}

@test "Multiple queries in succession" {
    # Verify server handles multiple rapid queries without issues
    for i in {1..10}; do
        result=$(query_a "www.example.com.")
        assert_equal "$result" "192.0.2.10"
    done
}

@test "Query with invalid record type number" {
    # TYPE99999 is not a valid DNS record type
    run dig @127.0.0.1 -p "$LRMDNS_PORT" www.example.com. TYPE99999 +short
    # Server should handle gracefully, may return empty or error
    assert_success
}

@test "Query for zone apex with different record types" {
    # Query for SOA at zone apex
    run dig @127.0.0.1 -p "$LRMDNS_PORT" example.com. SOA +short
    assert_success
    assert_output --partial "ns1.example.com"

    # Query for NS at zone apex
    run dig @127.0.0.1 -p "$LRMDNS_PORT" example.com. NS +short
    assert_success
    assert_output --partial "ns1.example.com"

    # Query for MX at zone apex (zone apex has MX record)
    run dig @127.0.0.1 -p "$LRMDNS_PORT" example.com. MX +short
    assert_success
    assert_output --partial "mail.example.com"
}

@test "Query with EDNS0 - small buffer size" {
    # Query with very small EDNS buffer
    result=$(query_edns "www.example.com." 512)
    assert_equal "$result" "192.0.2.10"
}

@test "Query with EDNS0 - large buffer size" {
    # Query with large EDNS buffer
    result=$(query_edns "www.example.com." 4096)
    assert_equal "$result" "192.0.2.10"
}

@test "TCP query after UDP query" {
    # First query over UDP
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"

    # Then query over TCP
    result=$(query_tcp "www.example.com." A)
    assert_equal "$result" "192.0.2.10"
}

@test "Concurrent TCP and UDP queries" {
    # Query over UDP
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"

    # Query over TCP
    result=$(query_tcp "www.example.com." A)
    assert_equal "$result" "192.0.2.10"

    # Server should still be responsive
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"
}

@test "Query for ANY type" {
    # Query for ANY should return records or handle gracefully
    # Modern DNS servers may not implement ANY queries (RFC 8482)
    run dig @127.0.0.1 -p "$LRMDNS_PORT" www.example.com. ANY
    assert_success
    # Server should respond (even if empty), not crash
}

@test "Server survives rapid connection attempts" {
    # Make 20 rapid queries to test server stability
    for i in {1..20}; do
        query_a "www.example.com." >/dev/null 2>&1
    done

    # Server should still be responsive
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"
}

@test "Query for PTR record type" {
    # Even though we don't have PTR records in basic zone, server should handle the query type
    local rcode=$(get_rcode "1.2.0.192.in-addr.arpa." PTR)
    # Should return REFUSED (not in our zone) or NXDOMAIN
    assert [ "$rcode" = "REFUSED" -o "$rcode" = "NXDOMAIN" ]
}

@test "Underscore in domain name (SRV records)" {
    # Underscores are allowed in DNS for SRV records like _http._tcp.example.com
    local rcode=$(get_rcode "_http._tcp.example.com." SRV)
    # Should handle gracefully - NXDOMAIN since we don't have this record
    assert_equal "$rcode" "NXDOMAIN"
}

@test "Server remains stable after errors" {
    # Generate several error conditions
    get_rcode "nonexistent.example.com." A >/dev/null 2>&1
    get_rcode "www.google.com." A >/dev/null 2>&1
    # Skip invalid..domain.com as dig validates it client-side

    # Server should still handle valid queries correctly
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"
}
