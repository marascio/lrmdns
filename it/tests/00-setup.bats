#!/usr/bin/env bats

@test "dig is installed" {
    command -v dig
}

@test "required fixtures exist" {
    [ -f "fixtures/zones/basic.zone" ]
    [ -f "fixtures/configs/basic.yaml" ]
}
