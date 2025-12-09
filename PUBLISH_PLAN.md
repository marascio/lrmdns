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

## Phase 2: README Updates ✅ COMPLETE

**Goal:** Fix placeholders and outdated information in README.md

### Tasks:
- [x] Fix installation section
  - Changed `git clone <repository>` to `git clone https://github.com/marascio/lrmdns.git`
- [x] Fix license section
  - Changed "MIT (or your preferred license)" to "MIT"
- [x] Update "Current Limitations" section
  - Removed outdated claims about zone reloading and rate limiting
  - Both features are now implemented
- [x] Add badges at top of README
  - License badge (MIT)
  - Rust version badge (1.88.0+ for edition 2024)
  - Build status badge (will activate after CI runs on GitHub)
- [x] Update Prerequisites section
  - Updated Rust version requirement to 1.88.0+ with edition 2024 note

**Changes Made:** 5 sections updated in README.md

---

## Phase 3: GitHub-Specific Files ✅ COMPLETE

**Goal:** Add GitHub community files and CI/CD

### 3.1: GitHub Workflows (CI/CD)

**Files created:**
- [x] `.github/workflows/ci.yml` - Main CI pipeline
  - Runs `cargo test` (debug + release) on push/PR
  - Runs `cargo clippy -- -D warnings`
  - Runs `cargo fmt --check`
  - Runs integration tests (`cd it && ./run-tests.sh`)
  - Matrix: Ubuntu latest, macOS latest
  - Rust version: stable
  - Includes code coverage with tarpaulin/codecov
  - Cargo caching for faster builds

### 3.2: Issue Templates

**Files created:**
- [x] `.github/ISSUE_TEMPLATE/bug_report.md` - Detailed bug report template
- [x] `.github/ISSUE_TEMPLATE/feature_request.md` - Feature request template
- [x] `.github/ISSUE_TEMPLATE/config.yml` - Links to security advisories and discussions

### 3.3: Pull Request Template

**Files created:**
- [x] `.github/PULL_REQUEST_TEMPLATE.md`
  - Comprehensive checklist for tests, formatting, documentation
  - Type of change selection
  - Performance impact section
  - Breaking changes section

### 3.4: Contributing Guidelines

**Files created:**
- [x] `CONTRIBUTING.md`
  - Complete development workflow and setup instructions
  - Testing guide (unit, integration, property-based)
  - Code style guidelines (rustfmt, clippy)
  - Commit message format (no emojis per user preference)
  - Branch naming conventions
  - PR process and review requirements

**Files Created:** 6 new files (673 lines total)

---

## Phase 4: SECURITY.md Enhancement ✅ COMPLETE

**Goal:** Add actual contact information

### Tasks:
- [x] Update SECURITY.md line 20
  - Using GitHub Security Advisories as primary contact method
  - Provides private disclosure and coordinated response
  - Automatic notification to maintainers

**Changes Made:** Updated contact section with GitHub Security Advisories

---

## Phase 5: Final Quality Checks ✅ COMPLETE

**Goal:** Verify everything works before publication

### Tasks:
- [x] Run full test suite one more time
  - `cargo build && cargo build --release` ✅
  - `cargo test && cargo test --release` ✅ (122 tests passed)
  - `cd it && ./run-tests.sh` ✅ (69 integration tests passed)
- [x] Run clippy with no warnings
  - `cargo clippy -- -D warnings` ✅
- [x] Run rustfmt check
  - `cargo fmt --check` ✅
- [x] Verify all links in documentation work
  - README.md links ✅
  - CHANGELOG.md links ✅
  - SECURITY.md links ✅
- [x] Check for any remaining placeholders
  - No TODO/FIXME in source code ✅
  - "YOUR_USERNAME" in CONTRIBUTING.md is intentional for contributors ✅
- [x] Review CHANGELOG.md for completeness ✅
- [x] Review Cargo.toml keywords and categories for discoverability ✅

**Status:** All quality checks passed successfully

---

## Phase 6: Publication - Ready for Manual Steps

**Goal:** Push to GitHub and set up repository

### Automated Preparation (Complete):
- [x] Git tag v0.1.0 created locally with release notes
- [x] Created GITHUB_PUBLICATION_STEPS.md with detailed instructions

### Manual Steps Required (Follow GITHUB_PUBLICATION_STEPS.md):
- [ ] Create repository on GitHub: `marascio/lrmdns`
  - Make it public
  - Don't initialize with README (we have one)
- [ ] Add remote and push
  ```bash
  git remote add origin https://github.com/marascio/lrmdns.git
  git push -u origin main --tags
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
  - Tag: `v0.1.0` (already created locally)
  - Title: "lrmdns v0.1.0 - Initial Release"
  - Body: Copy from GITHUB_PUBLICATION_STEPS.md or CHANGELOG.md
- [ ] Verify CI runs successfully on GitHub Actions
- [ ] Add GitHub repository social preview image (optional)

**Status:** Ready to publish - see GITHUB_PUBLICATION_STEPS.md for step-by-step instructions

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

- ✅ Phase 1: Complete - Documentation cleanup
- ✅ Phase 2: Complete - README updates
- ✅ Phase 3: Complete - GitHub community files and CI/CD
- ✅ Phase 4: Complete - SECURITY.md enhancement
- ✅ Phase 5: Complete - Final quality checks
- 🔄 Phase 6: Ready for manual steps - Publish to GitHub (see GITHUB_PUBLICATION_STEPS.md)
- ⏳ Phase 7: Not started - Post-publication polish

---

## Notes

- Edition 2024 is used, which requires Rust 1.88.0+ (released Nov 2024)
- Project has excellent test coverage: 122 unit tests + 69 integration tests
- Pre-commit hooks already configured for rustfmt and clippy
- Property-based testing already implemented with proptest
