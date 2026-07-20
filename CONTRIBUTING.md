# Contributing to Sentinel AI

Thank you for your interest in contributing to Sentinel AI! This document provides guidelines and information for contributors.

## How to Contribute

### Reporting Bugs

1. Check existing issues to avoid duplicates
2. Open a new issue with:
   - Clear description of the bug
   - Steps to reproduce
   - Expected vs actual behavior
   - Environment details (OS, Rust version)

### Suggesting Features

1. Open an issue with the `enhancement` label
2. Describe the use case and benefits
3. Include examples if possible

### Submitting Code

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass (`cargo test --workspace`)
6. Run clippy (`cargo clippy --workspace`)
7. Format code (`cargo fmt`)
8. Commit with clear messages
9. Push to your branch
10. Open a Pull Request

## Development Setup

### Prerequisites

- Rust 1.75+ (latest stable recommended)
- Cargo
- Git

### Getting Started

```bash
# Clone the repository
git clone https://github.com/user/sentinel-ai.git
cd sentinel-ai

# Build the project
cargo build --release

# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace

# Format code
cargo fmt
```

### Project Structure

```
sentinel/
├── crates/
│   ├── sentinel-core/        # Core types and data model
│   ├── sentinel-common/      # Shared utilities
│   ├── sentinel-demo/        # Demo file parsing
│   ├── sentinel-events/      # Event pipeline
│   ├── sentinel-world/       # World state reconstruction
│   ├── sentinel-map/         # Map geometry
│   ├── sentinel-visibility/  # Visibility engine
│   ├── sentinel-features/    # Feature extraction
│   ├── sentinel-analysis/    # Behavior analysis
│   ├── sentinel-evidence/    # Evidence collection
│   ├── sentinel-report/      # Report generation
│   ├── sentinel-datasets/    # Dataset management
│   ├── sentinel-source2/     # CS2 demo adapter
│   ├── sentinel-validation/  # Validation harness
│   └── sentinel-cli/         # Command-line interface
└── docs/                     # Documentation
```

## Code Style

### Rust

- Follow Rust API Guidelines
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Write documentation comments for public items
- Add tests for new functionality

### Commit Messages

Use conventional commits:

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation
- `style:` Formatting
- `refactor:` Code refactoring
- `test:` Adding tests
- `chore:` Maintenance

Examples:
```
feat: add visibility engine for line-of-sight checks
fix: correct rotation justification calculation
docs: update README with installation instructions
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p sentinel-core

# Run with output
cargo test -- --nocapture
```

### Writing Tests

- Add unit tests for new functions
- Use descriptive test names
- Test edge cases and error conditions
- Aim for high coverage on critical code

### Golden Tests

For features that produce deterministic output:
1. Create a test with known input
2. Verify expected output
3. Update golden files when behavior intentionally changes

## Pull Request Process

1. Update documentation if needed
2. Add tests for new features
3. Ensure CI passes
4. Request review from maintainers
5. Address feedback promptly

## Code Review Checklist

- [ ] Code follows Rust guidelines
- [ ] Tests are included
- [ ] Documentation is updated
- [ ] No breaking changes (or version bumped)
- [ ] Performance considerations addressed
- [ ] Security implications reviewed

## Getting Help

- Open an issue for questions
- Join discussions in existing issues
- Check documentation in `docs/`

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
