#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

# Tests for NAPTR, TLSA, and SSHFP record types

setup() {
    start_server fixtures/configs/extended-records.yaml
}

teardown() {
    cleanup_server
}

@test "NAPTR record query for SIP service" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" _sip._tcp.example.com. NAPTR +short)
    assert [ -n "$result" ]
    echo "$result" | grep -q "E2U+sip"
}

@test "NAPTR record contains order and preference" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" _sip._tcp.example.com. NAPTR +short)
    # Should have two NAPTR records with order 100
    echo "$result" | grep -q "^100 10"
    echo "$result" | grep -q "^100 20"
}

@test "NAPTR record for email service" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" _sip._tcp.example.com. NAPTR +short)
    echo "$result" | grep -q "E2U+email"
}

@test "TLSA record query for HTTPS" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" _443._tcp.example.com. TLSA +short)
    assert [ -n "$result" ]
    # Should contain usage=3, selector=1, matching=1
    echo "$result" | grep -q "^3 1 1"
}

@test "TLSA record contains certificate hash" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" _443._tcp.example.com. TLSA +short)
    # Should contain the certificate hash (at least part of it)
    echo "$result" | grep -qi "D2ABDE240D7CD3EE"
}

@test "SSHFP record query for host" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" server.example.com. SSHFP +short)
    assert [ -n "$result" ]
    # Should have at least one SSHFP record (algorithm 1, hash type 2)
    echo "$result" | grep -q "1 2"
}

@test "SSHFP record for Ed25519 key" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" server.example.com. SSHFP +short)
    # Should contain Ed25519 (algorithm 4) record
    echo "$result" | grep -q "4 2"
}

@test "SSHFP record contains fingerprint" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" server.example.com. SSHFP +short)
    # Should contain hex fingerprint
    echo "$result" | grep -qi "123456789ABCDEF"
}

@test "Query for unsupported record type returns empty" {
    # HINFO is not yet supported
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" server.example.com. HINFO +short)
    assert_equal "$result" ""
}

@test "NAPTR query over TCP" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" +tcp _sip._tcp.example.com. NAPTR +short)
    assert [ -n "$result" ]
    echo "$result" | grep -q "E2U+sip"
}

@test "TLSA query over TCP" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" +tcp _443._tcp.example.com. TLSA +short)
    assert [ -n "$result" ]
    echo "$result" | grep -q "^3 1 1"
}

@test "SSHFP query over TCP" {
    result=$(dig @127.0.0.1 -p "$LRMDNS_PORT" +tcp server.example.com. SSHFP +short)
    assert [ -n "$result" ]
    echo "$result" | grep -q "1 2"
}
