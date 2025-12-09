# GitHub Publication Plan for lrmdns

This document outlines the steps needed to prepare lrmdns for public GitHub publication.

## Status: In Progress

**Last Updated:** 2025-12-08

---

## Phase 1: Documentation Cleanup ✅ COMPLETE

**Goal:** Ensure all documentation is accurate and professional

### Tasks:
- [x] Add LICENSE file (MIT)
- [x] Add SECURITY.md with security policy
- [x] Add CHANGELOG.md in Keep-a-Changelog format
- [x] Update Cargo.toml with complete metadata
- [x] Update .gitignore (remove Cargo.lock, add secrets patterns)
- [x] Add Cargo.lock for reproducible builds
- [x] Update GitHub username references to marascio
- [x] Run all tests to verify (122 unit + 69 integration)

---

## Phase 2: README Updates

**Goal:** Fix placeholders and outdated information in README.md

### Tasks:
- [ ] Fix installation section
  - Line 43: Change `git clone <repository>` to `git clone https://github.com/marascio/lrmdns.git`
- [ ] Fix license section
  - Line 299: Change "MIT (or your preferred license)" to "MIT"
- [ ] Update "Current Limitations" section
  - Line 280-282: Remove outdated claim "No zone reloading without restart"
  - Zone reloading is implemented via SIGHUP signal
- [ ] Add badges at top of README
  - License badge
  - Rust version badge (1.88.0+ for edition 2024)
  - Build status badge (after CI setup)

**Estimated Changes:** 4-5 lines in README.md

---

## Phase 3: GitHub-Specific Files

**Goal:** Add GitHub community files and CI/CD

### 3.1: GitHub Workflows (CI/CD)

**Files to create:**
- [ ] `.github/workflows/ci.yml` - Main CI pipeline
  - Run `cargo test` on push/PR
  - Run `cargo clippy` with warnings as errors
  - Run `cargo fmt --check`
  - Run integration tests (`cd it && ./run-tests.sh`)
  - Matrix: Linux (Ubuntu latest), macOS (latest)
  - Rust version: stable
- [ ] `.github/workflows/release.yml` - Release automation (optional for now)

### 3.2: Issue Templates

**Files to create:**
- [ ] `.github/ISSUE_TEMPLATE/bug_report.md`
- [ ] `.github/ISSUE_TEMPLATE/feature_request.md`
- [ ] `.github/ISSUE_TEMPLATE/config.yml` - Template chooser config

### 3.3: Pull Request Template

**Files to create:**
- [ ] `.github/PULL_REQUEST_TEMPLATE.md`
  - Checklist for tests, formatting, documentation
  - Link to related issues

### 3.4: Contributing Guidelines

**Files to create:**
- [ ] `CONTRIBUTING.md`
  - How to set up development environment
  - How to run tests (unit, integration, property-based)
  - Code style guidelines (rustfmt, clippy)
  - How to submit issues and PRs
  - Reference to pre-commit hooks
  - Development workflow (branch → test → commit → PR)

**Estimated Files:** 6 new files in .github/ + CONTRIBUTING.md

---

## Phase 4: SECURITY.md Enhancement

**Goal:** Add actual contact information

### Tasks:
- [ ] Update SECURITY.md line 20
  - Add security contact email or GitHub security advisory URL
  - Options:
    - Personal email
    - GitHub security advisories only
    - Create security@lrmdns.com (if domain available)

**Estimated Changes:** 1 line

---

## Phase 5: Final Quality Checks

**Goal:** Verify everything works before publication

### Tasks:
- [ ] Run full test suite one more time
  - `cargo build && cargo build --release`
  - `cargo test && cargo test --release`
  - `cd it && ./run-tests.sh`
- [ ] Run clippy with no warnings
  - `cargo clippy -- -D warnings`
- [ ] Run rustfmt check
  - `cargo fmt --check`
- [ ] Verify all links in documentation work
  - README.md links
  - CHANGELOG.md links
  - SECURITY.md links
- [ ] Check for any remaining placeholders
  - `grep -r "TODO\|FIXME\|placeholder\|yourusername" . --exclude-dir=target --exclude-dir=.git --exclude-dir=bats`
- [ ] Review CHANGELOG.md for completeness
- [ ] Review Cargo.toml keywords and categories for discoverability

**Estimated Time:** 30 minutes of verification

---

## Phase 6: Publication

**Goal:** Push to GitHub and set up repository

### Tasks:
- [ ] Create repository on GitHub: `marascio/lrmdns`
  - Make it public
  - Don't initialize with README (we have one)
- [ ] Add remote and push
  ```bash
  git remote add origin https://github.com/marascio/lrmdns.git
  git push -u origin master
  ```
- [ ] Configure repository settings on GitHub:
  - Add description: "Lightweight authoritative-only DNS server written in Rust with DNSSEC support"
  - Add topics/tags: `rust`, `dns`, `dns-server`, `authoritative`, `dnssec`, `networking`, `async`
  - Enable Issues
  - Enable Discussions (optional)
  - Configure branch protection for master (optional):
    - Require PR reviews
    - Require status checks (CI) to pass
- [ ] Create initial release v0.1.0
  - Tag: `v0.1.0`
  - Title: "lrmdns v0.1.0 - Initial Release"
  - Body: Copy from CHANGELOG.md
  - Attach binary builds (optional)
- [ ] Verify CI runs successfully on GitHub Actions
- [ ] Add GitHub repository social preview image (optional)

**Estimated Time:** 15 minutes

---

## Phase 7: Post-Publication Polish (Optional)

**Goal:** Enhance discoverability and professionalism

### Tasks:
- [ ] Add examples/ directory
  - Example zone files for common scenarios
  - Example configuration files
  - Systemd service file example
- [ ] Create crates.io publication (separate plan needed)
- [ ] Set up GitHub Pages with rustdoc (optional)
- [ ] Add more badges to README
  - crates.io version/downloads (after publishing)
  - docs.rs badge (after crates.io)
- [ ] Set up dependabot
  - `.github/dependabot.yml`
  - Automated dependency updates
- [ ] Community engagement
  - Share on /r/rust
  - Share on DNS/networking forums
  - Add to awesome-rust lists

---

## Summary

### Must-Do Before Publication:
- Phase 2: README updates (4 tasks)
- Phase 3: GitHub files (7 files)
- Phase 4: SECURITY.md contact info (1 task)
- Phase 5: Final quality checks (6 tasks)
- Phase 6: Publish to GitHub (4 tasks)

### Total Estimated Work:
- **New files:** ~7
- **Modified files:** ~3
- **Time:** 2-3 hours for implementation + testing

### Can Be Done After Initial Publication:
- Phase 7: Post-publication polish

---

## Progress Tracking

- ✅ Phase 1: Complete
- ⏳ Phase 2: Not started
- ⏳ Phase 3: Not started
- ⏳ Phase 4: Not started
- ⏳ Phase 5: Not started
- ⏳ Phase 6: Not started
- ⏳ Phase 7: Not started

---

## Notes

- Edition 2024 is used, which requires Rust 1.88.0+ (released Nov 2024)
- Project has excellent test coverage: 122 unit tests + 69 integration tests
- Pre-commit hooks already configured for rustfmt and clippy
- Property-based testing already implemented with proptest
