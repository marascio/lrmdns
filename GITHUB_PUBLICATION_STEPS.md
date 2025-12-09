# GitHub Publication Steps for lrmdns

This document provides the exact steps to publish lrmdns to GitHub.

## Status: Ready to Publish

All preparation work is complete:
- ✅ Phase 1-5 completed (documentation, quality checks, CI/CD setup)
- ✅ Git tag v0.1.0 created locally
- ✅ All tests passing (122 unit + 69 integration)

## Step 1: Create GitHub Repository

1. Go to https://github.com/new
2. Configure the repository:
   - **Repository name**: `lrmdns`
   - **Description**: `Lightweight authoritative-only DNS server written in Rust with DNSSEC support`
   - **Visibility**: Public
   - **Do NOT initialize** with README, .gitignore, or license (we already have these)
3. Click "Create repository"

## Step 2: Add Remote and Push

Run these commands from your local repository:

```bash
# Add GitHub as the origin remote
git remote add origin https://github.com/marascio/lrmdns.git

# Push main branch and all tags
git push -u origin main --tags
```

This will:
- Push all commits to GitHub
- Push the v0.1.0 tag
- Set up main to track origin/main

## Step 3: Configure Repository Settings

Go to repository settings on GitHub:

### 3.1 General Settings
- Navigate to: Settings → General
- Verify description: "Lightweight authoritative-only DNS server written in Rust with DNSSEC support"

### 3.2 Topics/Tags
- Navigate to: Settings → General (top of page, next to About)
- Click the gear icon next to "About"
- Add topics: `rust`, `dns`, `dns-server`, `authoritative`, `dnssec`, `networking`, `async`

### 3.3 Features
- Navigate to: Settings → General → Features
- ✅ Enable **Issues**
- ✅ Enable **Discussions** (optional but recommended)
- ✅ Wikis can remain disabled

### 3.4 Branch Protection (Optional but Recommended)
- Navigate to: Settings → Branches
- Add rule for `main` branch:
  - ✅ Require pull request reviews before merging
  - ✅ Require status checks to pass before merging
    - Select: `Test (ubuntu-latest, stable)`, `Test (macos-latest, stable)`
  - ✅ Require branches to be up to date before merging

## Step 4: Create GitHub Release

1. Go to: https://github.com/marascio/lrmdns/releases/new
2. Click "Choose a tag" and select `v0.1.0`
3. Set release title: `lrmdns v0.1.0 - Initial Release`
4. Copy the following into the release description:

```markdown
A lightweight authoritative-only DNS server written in Rust.

## Core Features
- Authoritative-only DNS server with UDP and TCP support
- Async I/O using tokio for high concurrency
- Standard DNS record types: A, AAAA, NS, SOA, CNAME, MX, TXT, PTR, SRV, CAA
- DNSSEC support: DNSKEY, RRSIG, NSEC, DS
- Additional record types: NAPTR, TLSA, SSHFP
- CNAME chain resolution
- Wildcard record support
- EDNS0 support with up to 4096 byte UDP responses
- AXFR zone transfers over TCP
- Hot reload capability via SIGHUP signal
- Metrics collection and HTTP API
- Per-IP rate limiting

## Testing
- 122 unit tests (including 17 property-based tests with proptest)
- 69 integration tests using BATS framework
- Pre-commit hooks for code quality (rustfmt, clippy)

## Installation

```bash
git clone https://github.com/marascio/lrmdns.git
cd lrmdns
cargo build --release
```

See [README.md](https://github.com/marascio/lrmdns/blob/main/README.md) for full documentation and [CHANGELOG.md](https://github.com/marascio/lrmdns/blob/main/CHANGELOG.md) for complete release notes.
```

5. Check "Set as the latest release"
6. Click "Publish release"

## Step 5: Verify CI/CD

1. Go to: https://github.com/marascio/lrmdns/actions
2. Wait for the CI workflow to complete (triggered by the push)
3. Verify all jobs pass:
   - Test (ubuntu-latest)
   - Test (macos-latest)
   - Code Coverage

## Step 6: Final Verification

Check that everything looks correct:

- [ ] README displays properly on repository homepage
- [ ] Topics/tags are visible
- [ ] Issues are enabled
- [ ] CI badge shows passing status (may need to wait for first run)
- [ ] Release v0.1.0 is published
- [ ] License badge shows MIT
- [ ] All links in README work

## Next Steps (Optional - Phase 7)

After successful publication, consider:

- Publishing to crates.io
- Setting up GitHub Discussions
- Adding more examples
- Creating a project website with GitHub Pages
- Sharing on social media and forums (/r/rust, etc.)

## Troubleshooting

**If push fails with authentication error:**
```bash
# You may need to use SSH instead of HTTPS
git remote set-url origin git@github.com:marascio/lrmdns.git
git push -u origin main --tags
```

**If CI fails:**
- Check the Actions tab for error details
- Most common issues are covered by the pre-push testing we've done
- The CI runs the same tests we've verified locally

**If you need to update the tag:**
```bash
# Delete local tag
git tag -d v0.1.0

# Delete remote tag (if already pushed)
git push origin :refs/tags/v0.1.0

# Create new tag
git tag -a v0.1.0 -m "Updated release message"

# Push new tag
git push origin v0.1.0
```
