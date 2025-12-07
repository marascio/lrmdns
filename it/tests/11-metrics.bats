#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

setup() {
    start_server fixtures/configs/basic-with-api.yaml 15353
}

teardown() {
    stop_server
}

@test "Health endpoint returns healthy" {
    run curl -s http://127.0.0.1:18080/health
    assert_success
    echo "$output" | grep -q '"status":"healthy"'
}

@test "Metrics endpoint is accessible" {
    run curl -s http://127.0.0.1:18080/metrics
    assert_success
}

@test "Metrics endpoint returns JSON" {
    run curl -s http://127.0.0.1:18080/metrics
    assert_success
    echo "$output" | grep -q "total_queries"
}

@test "Query increments metrics counter" {
    # Make a query
    query_a "www.example.com."

    # Check metrics
    run curl -s http://127.0.0.1:18080/metrics
    assert_success
    echo "$output" | grep -q "total_queries"
}
