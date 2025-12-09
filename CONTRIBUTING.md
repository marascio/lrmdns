# Contributing to lrmdns

Thank you for your interest in contributing to lrmdns! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Code Style](#code-style)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Features](#suggesting-features)

## Code of Conduct

This project follows a simple code of conduct: be respectful, constructive, and collaborative. We welcome contributions from everyone.

## Getting Started

### Prerequisites

- Rust 1.88.0+ (required for edition 2024 features)
- Git
- dig (from BIND utilities) for testing

### Setting Up Your Development Environment

1. Fork the repository on GitHub
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/lrmdns.git
   cd lrmdns
   ```

3. Add the upstream repository:
   ```bash
   git remote add upstream https://github.com/marascio/lrmdns.git
   ```

4. Install pre-commit hooks (optional but recommended):
   ```bash
   # The .pre-commit-config.yaml is already configured
   # Pre-commit hooks run rustfmt and clippy automatically
   ```

5. Build the project:
   ```bash
   cargo build
   cargo build --release
   ```

6. Run tests to verify everything works:
   ```bash
   cargo test
   cd it && ./run-tests.sh
   ```

## Development Workflow

### Always Work in Branches

**NEVER commit directly to master.** Always create a feature branch:

```bash
git checkout -b feat/your-feature-name
# or
git checkout -b fix/bug-description
# or
git checkout -b docs/documentation-update
```

Branch naming conventions:
- `feat/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation changes
- `refactor/` - Code refactoring
- `test/` - Test additions or improvements
- `chore/` - Maintenance tasks

### Development Cycle

1. Create a branch
2. Make your changes
3. Write/update tests
4. Run the full test suite
5. Commit your changes
6. Push to your fork
7. Create a pull request

### Keeping Your Fork Updated

```bash
git fetch upstream
git checkout master
git merge upstream/master
git push origin master
```

## Testing

lrmdns has comprehensive testing requirements. **All tests must pass before a PR is merged.**

### Running Tests

#### Unit Tests

```bash
# Run all unit tests in debug mode
cargo test

# Run all unit tests in release mode
cargo test --release

# Run tests with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

#### Integration Tests

```bash
cd it
./run-tests.sh                    # All tests
./run-tests.sh --parallel         # Parallel execution
bats/bats-core/bin/bats tests/01-basic-queries.bats  # Single file
```

#### Property-Based Tests

Property tests run automatically with `cargo test`. They use the `proptest` crate and are defined in source files with `#[cfg(test)]`.

### Writing Tests

When adding new features:

1. **Add unit tests** in the same file as your code:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_your_feature() {
           // Test implementation
       }
   }
   ```

2. **Add integration tests** in `it/tests/`:
   - Use existing test files for similar functionality
   - Create new files for entirely new features
   - Follow the naming convention: `##-feature-name.bats`

3. **Add property tests** for protocol-level features:
   ```rust
   #[cfg(test)]
   mod proptests {
       use super::*;
       use proptest::prelude::*;

       proptest! {
           #[test]
           fn prop_your_property(input in any::<YourType>()) {
               // Property test
           }
       }
   }
   ```

## Code Style

### Rust Style

- Follow the official Rust style guide
- Use `rustfmt` for formatting: `cargo fmt`
- Fix all `clippy` warnings: `cargo clippy`

### Pre-Commit Checklist

Before committing, **always** verify:

```bash
# 1. Build without warnings
cargo build
cargo build --release

# 2. Run unit tests
cargo test
cargo test --release

# 3. Run integration tests
cd it && ./run-tests.sh && cd ..

# 4. Check formatting
cargo fmt --check

# 5. Check clippy
cargo clippy -- -D warnings
```

### Code Organization

- Keep functions focused and small
- Add comments for complex logic
- Use descriptive variable and function names
- Prefer explicit error handling over unwrap/expect
- Document public APIs with rustdoc comments

## Commit Guidelines

### Commit Message Format

```
<type>: <subject>

<body>

Co-Authored-By: Your Name <your.email@example.com>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `test`: Test additions or improvements
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `chore`: Maintenance tasks

### Examples

```
feat: add support for TLSA record type

Implements parsing and serving of TLSA records for DANE support.
Includes unit tests and integration tests.

Co-Authored-By: Your Name <your.email@example.com>
```

```
fix: correct CNAME chain resolution for wildcard records

Wildcard CNAME records were not being resolved correctly when
the chain included multiple levels. Updated the resolution logic
to handle wildcard CNAMEs at any depth.

Fixes #123

Co-Authored-By: Your Name <your.email@example.com>
```

### Important Notes

- **DO NOT** use emojis or unicode characters in commit messages
- **DO NOT** include "Generated with Claude Code" lines (user preference)
- Keep the subject line under 72 characters
- Use the imperative mood ("add feature" not "added feature")
- Reference issues with `Fixes #123` or `Relates to #456`

## Pull Request Process

### Before Submitting

1. Ensure all tests pass
2. Update documentation (README, code comments, etc.)
3. Update CHANGELOG.md if applicable
4. Rebase on latest master if needed
5. Ensure your branch has a clear, descriptive name

### Creating a Pull Request

1. Push your branch to your fork:
   ```bash
   git push origin feat/your-feature-name
   ```

2. Go to GitHub and create a pull request from your fork

3. Fill out the PR template completely:
   - Describe your changes
   - Link to related issues
   - Indicate what testing you've done
   - Check all applicable boxes

4. Wait for CI to pass
5. Address any review feedback

### PR Review Process

- At least one maintainer must approve
- All CI checks must pass
- No merge conflicts with master
- Code follows project style guidelines
- Tests adequately cover the changes

### After Your PR is Merged

- Delete your feature branch (both locally and on GitHub)
- Update your local master:
  ```bash
  git checkout master
  git pull upstream master
  git push origin master
  ```

## Reporting Bugs

### Before Reporting

1. Check existing issues to avoid duplicates
2. Verify the bug exists in the latest version
3. Try to isolate the bug with minimal reproduction steps

### Submitting a Bug Report

Use the [Bug Report template](.github/ISSUE_TEMPLATE/bug_report.md) and include:

- Clear description of the bug
- Steps to reproduce
- Expected vs actual behavior
- Environment details (OS, Rust version, lrmdns version)
- Configuration files (sanitized)
- Logs (with `RUST_BACKTRACE=1` if applicable)
- Query used (if DNS-related)

## Suggesting Features

### Before Suggesting

1. Check existing issues and discussions
2. Consider if the feature fits the project's scope
3. Think about the implementation approach

### Submitting a Feature Request

Use the [Feature Request template](.github/ISSUE_TEMPLATE/feature_request.md) and include:

- Clear description of the feature
- Problem it solves
- Proposed solution
- Alternative approaches considered
- Use cases
- Willingness to contribute implementation

## Questions?

- Open a [Discussion](https://github.com/marascio/lrmdns/discussions)
- Review existing issues and PRs
- Check the [README](README.md) and other documentation

## License

By contributing to lrmdns, you agree that your contributions will be licensed under the MIT License.
