#!/usr/bin/env bats

@test "lrmdns binary exists" {
    [ -f "../target/release/lrmdns" ] || [ -f "../target/debug/lrmdns" ]
}

@test "dig is installed" {
    command -v dig
}

@test "required fixtures exist" {
    [ -f "fixtures/zones/basic.zone" ]
    [ -f "fixtures/configs/basic.yaml" ]
}

@test "BATS framework is available" {
    [ -f "bats/bats-core/bin/bats" ]
}
