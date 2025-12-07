#!/usr/bin/env bats

load '../helpers/server'
load '../helpers/dns'
load '../bats/test_helper/bats-support/load'
load '../bats/test_helper/bats-assert/load'

# Tests for various SOA record format support

@test "Single-line SOA format" {
    # Create temporary config for this zone
    cat > /tmp/soa-single-line.yaml <<EOF
server:
  listen: "127.0.0.1:15354"
zones:
  - name: test.example.
    file: $(pwd)/fixtures/zones/soa-single-line.zone
EOF

    start_server /tmp/soa-single-line.yaml 15354

    # Query SOA record
    run dig @127.0.0.1 -p 15354 test.example. SOA +short
    assert_success
    assert_output --partial "ns1.test.example"
    assert_output --partial "hostmaster.test.example"
    assert_output --partial "2024120601"

    # Query A record to ensure zone works
    result=$(dig @127.0.0.1 -p 15354 www.test.example. A +short)
    assert_equal "$result" "192.0.2.100"

    stop_server
    rm -f /tmp/soa-single-line.yaml
}

@test "Multi-line SOA with detailed comments" {
    cat > /tmp/soa-multiline-comments.yaml <<EOF
server:
  listen: "127.0.0.1:15355"
zones:
  - name: test2.example.
    file: $(pwd)/fixtures/zones/soa-multiline-comments.zone
EOF

    start_server /tmp/soa-multiline-comments.yaml 15355

    # Query SOA record
    run dig @127.0.0.1 -p 15355 test2.example. SOA +short
    assert_success
    assert_output --partial "ns1.test2.example"
    assert_output --partial "admin.test2.example"
    assert_output --partial "2024120602"
    assert_output --partial "7200"
    assert_output --partial "3600"

    # Query MX record
    run dig @127.0.0.1 -p 15355 test2.example. MX +short
    assert_success
    assert_output --partial "mail.test2.example"

    stop_server
    rm -f /tmp/soa-multiline-comments.yaml
}

@test "Multi-line SOA compact (no comments)" {
    cat > /tmp/soa-multiline-compact.yaml <<EOF
server:
  listen: "127.0.0.1:15356"
zones:
  - name: test3.example.
    file: $(pwd)/fixtures/zones/soa-multiline-compact.zone
EOF

    start_server /tmp/soa-multiline-compact.yaml 15356

    # Query SOA record
    run dig @127.0.0.1 -p 15356 test3.example. SOA +short
    assert_success
    assert_output --partial "2024120603"
    assert_output --partial "14400"

    # Query NS record
    run dig @127.0.0.1 -p 15356 test3.example. NS +short
    assert_success
    assert_output --partial "ns1.test3.example"

    stop_server
    rm -f /tmp/soa-multiline-compact.yaml
}

@test "Multi-line SOA with mixed grouping" {
    cat > /tmp/soa-multiline-mixed.yaml <<EOF
server:
  listen: "127.0.0.1:15357"
zones:
  - name: test4.example.
    file: $(pwd)/fixtures/zones/soa-multiline-mixed.zone
EOF

    start_server /tmp/soa-multiline-mixed.yaml 15357

    # Query SOA record
    run dig @127.0.0.1 -p 15357 test4.example. SOA +short
    assert_success
    assert_output --partial "2024120604"
    assert_output --partial "10800"
    assert_output --partial "604800"

    # Query CNAME record
    run dig @127.0.0.1 -p 15357 ftp.test4.example. A +short
    assert_success
    assert_output --partial "192.0.2.90"

    stop_server
    rm -f /tmp/soa-multiline-mixed.yaml
}

@test "Multi-line SOA with tabs and mixed whitespace" {
    cat > /tmp/soa-tabs-spaces.yaml <<EOF
server:
  listen: "127.0.0.1:15358"
zones:
  - name: test5.example.
    file: $(pwd)/fixtures/zones/soa-tabs-and-spaces.zone
EOF

    start_server /tmp/soa-tabs-spaces.yaml 15358

    # Query SOA record
    run dig @127.0.0.1 -p 15358 test5.example. SOA +short
    assert_success
    assert_output --partial "2024120605"
    assert_output --partial "14400"
    assert_output --partial "7200"

    # Query A record
    result=$(dig @127.0.0.1 -p 15358 ns1.test5.example. A +short)
    assert_equal "$result" "192.0.2.91"

    stop_server
    rm -f /tmp/soa-tabs-spaces.yaml
}

@test "Standard multi-line SOA (basic.zone)" {
    # This tests the standard format we use in basic.zone
    cat > /tmp/basic-soa-test.yaml <<EOF
server:
  listen: "127.0.0.1:15359"
zones:
  - name: example.com.
    file: $(pwd)/fixtures/zones/basic.zone
EOF

    start_server /tmp/basic-soa-test.yaml 15359

    # Query SOA record
    run dig @127.0.0.1 -p 15359 example.com. SOA +short
    assert_success
    assert_output --partial "ns1.example.com"
    assert_output --partial "admin.example.com"
    assert_output --partial "2024010101"
    assert_output --partial "7200"
    assert_output --partial "86400"

    stop_server
    rm -f /tmp/basic-soa-test.yaml
}

@test "All SOA formats return valid authoritative responses" {
    # Test that all formats produce authoritative responses
    cat > /tmp/multi-zone-soa.yaml <<EOF
server:
  listen: "127.0.0.1:15360"
zones:
  - name: test.example.
    file: $(pwd)/fixtures/zones/soa-single-line.zone
  - name: test2.example.
    file: $(pwd)/fixtures/zones/soa-multiline-comments.zone
  - name: test3.example.
    file: $(pwd)/fixtures/zones/soa-multiline-compact.zone
EOF

    start_server /tmp/multi-zone-soa.yaml 15360

    # Check each zone has authoritative flag
    for zone in test.example test2.example test3.example; do
        run dig @127.0.0.1 -p 15360 $zone SOA +noall +comments
        assert_success
        assert_output --regexp "flags:.*aa"
    done

    stop_server
    rm -f /tmp/multi-zone-soa.yaml
}
