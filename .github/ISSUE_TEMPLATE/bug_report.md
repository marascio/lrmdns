---
name: Bug Report
about: Report a bug or unexpected behavior
title: '[BUG] '
labels: bug
assignees: ''
---

## Bug Description

A clear and concise description of what the bug is.

## To Reproduce

Steps to reproduce the behavior:
1. Start server with '...'
2. Send query '...'
3. Observe '...'
4. See error

## Expected Behavior

A clear and concise description of what you expected to happen.

## Actual Behavior

What actually happened.

## Environment

- **lrmdns version**: (e.g., 0.1.0 or commit hash)
- **OS**: (e.g., Ubuntu 22.04, macOS 14.0)
- **Rust version**: (output of `rustc --version`)
- **Installation method**: (built from source, binary release)

## Configuration

```yaml
# Paste relevant parts of your lrmdns.yaml here
```

## Zone File (if relevant)

```
# Paste relevant parts of your zone file here
```

## Logs

```
# Paste relevant log output here
# Include full backtrace if available (RUST_BACKTRACE=1)
```

## Query Used

```bash
# Example dig command used
dig @127.0.0.1 -p 5353 example.com A
```

## Additional Context

Add any other context about the problem here (e.g., related issues, potential causes, workarounds found).
