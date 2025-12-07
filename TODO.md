# TODO

## Bugs

### Zone Parser: Multi-line SOA Record Support
**Priority**: Medium
**File**: `src/zone.rs`

The zone parser currently does not support the standard multi-line SOA record format with parentheses and inline comments.

**Current behavior**: Parser fails with "Zone file must contain an SOA record" and warns "Unsupported record type ;" for comment lines.

**Expected behavior**: Should parse standard multi-line SOA records like:
```dns
@ IN SOA ns1.example.com. admin.example.com. (
    2024010101  ; Serial
    7200        ; Refresh
    3600        ; Retry
    1209600     ; Expire
    86400       ; Minimum TTL
)
```

**Workaround**: Currently must use single-line format:
```dns
@ IN SOA ns1.example.com. admin.example.com. 2024010101 7200 3600 1209600 86400
```

**Impact**: Compatibility issue - multi-line SOA with comments is the standard format used in BIND and most DNS servers.

**Discovered**: During integration test implementation (2025-12-07)
