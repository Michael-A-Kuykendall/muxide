# Contributing to Muxide

Thank you for your interest in contributing to Muxide! This document provides guidelines and information for contributors.

## Code of Conduct

Be respectful and constructive. We're all here to build great software.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/muxide.git`
3. Create a branch: `git checkout -b feature/your-feature`
4. Make your changes
5. Run tests: `cargo test`
6. Submit a pull request

## Development Setup

```bash
# Clone and enter the repo
git clone https://github.com/Michael-A-Kuykendall/muxide.git
cd muxide

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy -- -D warnings

# Generate coverage report (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

## Pull Request Guidelines

### Before Submitting

- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] New code has tests
- [ ] Documentation is updated if needed

### PR Title Format

Use conventional commits:
- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation only
- `test:` Test changes
- `refactor:` Code refactoring
- `perf:` Performance improvement

### What We Look For

1. **Zero dependencies** - We don't add runtime dependencies
2. **MSRV 1.70** - Use only features available in Rust 1.70
3. **Test coverage** - New code should have property tests
4. **Documentation** - Public APIs need doc comments

## Architecture Overview

```
src/
├── api.rs           # Public MuxerBuilder interface
├── mp4.rs           # MP4/ISOBMFF box construction
├── aac.rs           # AAC ADTS parsing
├── fragmented.rs    # Fragmented MP4 support
├── invariant_ppt.rs # Testing infrastructure
└── lib.rs           # Module exports
```

## Testing Philosophy

We use **Invariant PPT** (Property-based Testing):

1. **Property tests** - Use `proptest` for edge cases
2. **Invariants** - Use `assert_invariant!()` for runtime checks
3. **Contract tests** - Verify invariants are enforced

Example property test:
```rust
proptest! {
    #[test]
    fn pts_always_increases(frames in prop::collection::vec(any::<u64>(), 1..100)) {
        // Property: PTS must always be monotonically increasing
    }
}
```

## Reporting Issues

When reporting bugs, please include:
- Rust version (`rustc --version`)
- OS and version
- Minimal reproduction code
- Expected vs actual behavior

## Feature Requests

We welcome feature requests! Please:
- Check existing issues first
- Describe the use case
- Explain why it fits Muxide's scope (see `docs/charter.md`)

## License

By contributing, you agree that your contributions will be licensed under the same MIT OR Apache-2.0 dual license as the project.

## Questions?

Open an issue with the `question` label.
