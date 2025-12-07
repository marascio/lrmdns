#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../helpers/assertions'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

setup() {
    start_server fixtures/configs/basic.yaml 15353
}

teardown() {
    stop_server
}

@test "A record query returns correct IP" {
    result=$(query_a "www.example.com.")
    assert_equal "$result" "192.0.2.10"
}

@test "AAAA record query returns IPv6" {
    result=$(query_aaaa "www.example.com.")
    assert_equal "$result" "2001:db8::10"
}

@test "MX record query returns mail server" {
    result=$(query_mx "example.com.")
    echo "$result" | grep -q "mail.example.com"
}

@test "TXT record query returns SPF record" {
    result=$(query_txt "example.com.")
    echo "$result" | grep -q "v=spf1"
}

@test "NS record query returns nameservers" {
    result=$(query_ns "example.com.")
    echo "$result" | grep -q "ns1.example.com"
}

@test "Non-existent domain returns NXDOMAIN" {
    assert_rcode "nonexistent.example.com." "NXDOMAIN"
}

@test "Response has authoritative flag" {
    run is_authoritative "www.example.com."
    assert_success
}

@test "CNAME record follows chain" {
    result=$(query_a "ftp.example.com.")
    echo "$result" | grep -q "192.0.2.10"
}
