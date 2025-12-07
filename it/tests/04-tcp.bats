#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

setup() {
    start_server fixtures/configs/basic.yaml 15353
}

teardown() {
    stop_server
}

@test "A record query over TCP returns correct IP" {
    result=$(query_tcp "www.example.com." A)
    assert_equal "$result" "192.0.2.10"
}

@test "MX record query over TCP" {
    result=$(query_tcp "example.com." MX)
    echo "$result" | grep -q "mail.example.com"
}

@test "Large query over TCP" {
    result=$(query_tcp "www.example.com." A)
    [ -n "$result" ]
}
