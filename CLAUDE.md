# Claude Code Generation Guidelines for Rust Projects

## Language & Project Context

This is a Rust project. Always consider Rust-specific idioms: always validate suggestions against Rust's ownership/borrowing rules before recommending removal of .clone(), .to_string(), or similar patterns, verify numeric types and bit widths before flagging values as errors, and suggest running `cargo clippy` when unsure about a suggestion.

## Code Review

When asked to review a file, review the file directly by reading it. Do NOT attempt to use `gh` CLI, PR numbers, or any PR-based review workflow. Just read the file and provide a code review.

When providing code reviews, deliver the complete review in one response. Do not stop partway through. If the review is long, prioritize critical issues first, then minor suggestions.

## Project Overview

swyh-rs is a "Stream What You Hear" application: it captures audio from the system's default output device (via CPAL/WASAPI loopback on Windows, ALSA/PipeWire on Linux) and streams it over HTTP to DLNA/UPnP renderers discovered via SSDP. It has both a GUI binary (FLTK) and a headless CLI binary. Key subsystems:

- **Audio capture**: `src/audio/` — CPAL-based capture, sample conversion, FLAC/WAV/MP3 encoding
- **HTTP streaming server**: `src/server/` — tiny HTTP server that pushes the audio stream to renderers
- **Renderer control**: `src/renderers/` — SSDP discovery and UPnP AV/OpenHome renderer control
- **UI**: `src/ui/mainform.rs` — FLTK tabbed GUI (Audio, Network, App, Status tabs)
- **Config**: `src/utils/configuration.rs` — INI-based persistent config via `serde`
- **CLI**: `src/bin/swyh-rs-cli.rs`, CLI args parsed with `lexopt`
- **i18n**: `src/utils/i18n.rs` — `fluent-templates` with `set_use_isolating(false)` (avoids bidi chars in FLTK labels)

## Core Architecture Principles

### 1. Error Handling & Resource Management

- **Use Result types**: Prefer `Result<T, E>` over panics for recoverable errors
- **Explicit error handling**: Use `?` operator and proper error propagation
- **RAII pattern**: Rust's ownership system handles resource cleanup automatically
- **Custom error types**: Create domain-specific error types using `thiserror` or `anyhow`

```rust
// Good example
use anyhow::{Context, Result};

fn process_file(path: &str) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path))?;
    
    // Process content...
    Ok(content)
}
```

### 2. Concurrency & Thread Safety

- **Ownership model**: Leverage Rust's ownership system for thread safety
- **Thread-based concurrency**: This project uses OS threads, not async/await — do not introduce `tokio` or `async-std`
- **Channel communication**: Use `crossbeam-channel` for thread communication
- **Mutex/RwLock**: Use for shared mutable state when necessary

### 3. Configuration & Dependency Injection

- **Serde configuration**: Use `serde` for serialization/deserialization
- **Dependency injection**: Pass dependencies explicitly through constructors
- **Feature flags**: Use Cargo features for conditional compilation

## File and Directory Structure

### Standard Layout

```text
swyh-rs/
├── src/
│   ├── lib.rs
│   ├── bin/
│   │   ├── swyh-rs.rs         # GUI binary entry point
│   │   └── swyh-rs-cli.rs     # Headless CLI binary
│   ├── audio/                 # CPAL capture, sample conversion, encoding
│   ├── enums/                 # Shared message/streaming enums
│   ├── globals/               # Static globals and shared state
│   ├── renderers/             # SSDP discovery, UPnP/OpenHome control
│   ├── server/                # HTTP streaming server
│   ├── ui/                    # FLTK GUI (mainform.rs)
│   └── utils/                 # Config, CLI parsing, i18n, logging, etc.
├── assets/                    # Icons and static assets
├── locales/                   # Fluent (.ftl) i18n files
├── tray_icon/                 # Windows tray icon helper (Python)
├── Cargo.toml
└── Cargo.lock
```

### File Naming Conventions

- **Rust files**: Use snake_case (e.g., `user_service.rs`, `auth_handler.rs`)
- **Test files**: Integration tests in `tests/` directory
- **Module files**: `mod.rs` for module declarations
- **Binary targets**: Place in `src/bin/` for additional executables

## Code Style & Standards

### Documentation

- **Rustdoc comments**: Use `///` for public API documentation
- **Module documentation**: Document modules with `//!` at the top
- **Examples**: Include code examples in documentation
- **Cargo.toml metadata**: Include proper project metadata

```rust
/// Processes user authentication requests.
///
/// # Arguments
///
/// * `username` - The user's username
/// * `password` - The user's password
///
/// # Returns
///
/// Returns `Ok(User)` if authentication succeeds, or `Err(AuthError)` if it fails.
///
/// # Examples
///
/// ```
/// let user = authenticate("alice", "secret123")?;
/// println!("Welcome, {}!", user.name);
/// ```
pub fn authenticate(username: &str, password: &str) -> Result<User, AuthError> {
    // Implementation...
}
```

### Logging Standards

- **Structured logging**: Use the `log` crate (`log::info!`, `log::debug!`, etc.) — do not use `tracing`
- **Log levels**: Use appropriate levels (trace, debug, info, warn, error)
- **Performance**: Use logging guards for expensive operations

### Testing Requirements

- **Unit tests**: Include `#[cfg(test)]` modules in source files
- **Integration tests**: Place in `tests/` directory
- **Property testing**: Use `proptest` for property-based testing
- **Mocking**: Use `mockall` for mocking dependencies
- **Coverage**: Use `cargo tarpaulin` for code coverage

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_basic_functionality() {
        let result = process_data("test input");
        assert!(result.is_ok());
    }

    proptest! {
        #[test]
        fn test_property_based(input in ".*") {
            let result = validate_input(&input);
            prop_assert!(result.is_ok() || result.is_err());
        }
    }
}
```

## Platform-Specific Considerations

### Cross-Platform Compatibility

- **Conditional compilation**: Use `cfg` attributes for platform-specific code
- **Path handling**: Use `std::path::Path` for cross-platform path operations
- **Feature detection**: Use `cfg!` macro for runtime feature detection

```rust
#[cfg(target_os = "windows")]
fn platform_specific_function() {
    // Windows-specific implementation
}

#[cfg(unix)]
fn platform_specific_function() {
    // Unix-specific implementation
}
```

## Common Patterns & Anti-Patterns

### Do's

- ✅ Use `Result<T, E>` for error handling
- ✅ Leverage ownership and borrowing for memory safety
- ✅ Use iterators instead of manual loops
- ✅ Implement `Display` and `Debug` traits appropriately
- ✅ Use `clippy` for code quality checks
- ✅ Write comprehensive tests and documentation
- ✅ Use `serde` for serialization needs
- ✅ Follow Rust naming conventions

### Don'ts

- ❌ Don't use `unwrap()` to silently ignore errors — only use it when the invariant is statically obvious (e.g. after `is_some()`, or on `Mutex::lock()` where poisoning would itself be a bug)
- ❌ Don't use `panic!` for normal error flow
- ❌ Don't ignore compiler warnings
- ❌ Don't use `unsafe` without careful consideration
- ❌ Don't create unnecessary allocations
- ❌ Don't write untested code
- ❌ Don't use global mutable state

## Development Workflow

### Feature Development

1. **Design API**: Define public interfaces and types first
2. **Write tests**: Write failing tests before implementation
3. **Implement incrementally**: Build in small, testable increments
4. **Document thoroughly**: Include examples and edge cases
5. **Commit atomically**: Make small, focused commits

### Code Review Checklist

- [ ] Follows Rust idioms and conventions
- [ ] Proper error handling with `Result` types
- [ ] Comprehensive test coverage
- [ ] Clear documentation and examples
- [ ] No compiler warnings or clippy lints
- [ ] Appropriate use of lifetimes and borrowing
- [ ] Performance considerations addressed
- [ ] Security best practices followed

## Performance Considerations

### Memory Management

- **Zero-cost abstractions**: Leverage Rust's zero-cost abstractions
- **Avoid unnecessary allocations**: Use string slices over owned strings when possible
- **Iterator chains**: Use iterator adaptors for efficient data processing
- **Profiling**: Use `perf` and `flamegraph` for performance analysis

### Concurrency Performance

- **Thread pools**: Spawn threads for long-running tasks (audio capture, HTTP serving, SSDP)
- **Buffering**: Use appropriate buffer sizes for I/O operations
- **Channels**: Prefer `crossbeam-channel` over `std::sync::mpsc` for performance

## Security & Privacy

### Data Handling

- **Input validation**: Validate all external inputs
- **Sanitization**: Sanitize data before processing
- **Secure defaults**: Use secure defaults for configurations
- **Secrets management**: Never hardcode secrets in source code

### Memory Safety

- **Ownership system**: Rust's ownership prevents many security issues
- **Bounds checking**: Array bounds are checked at runtime
- **Type safety**: Use strong typing to prevent logic errors
- **Unsafe code**: Minimize and carefully review any `unsafe` blocks

## Tooling & Development Environment

### Essential Tools

- **Rustfmt**: Code formatting with `cargo fmt`
- **Clippy**: Linting with `cargo clippy`
- **Cargo**: Build system and package manager
- **Rust analyzer**: IDE integration for better development experience

### Code Search & Analysis

- **Ripgrep**: Fast text search with `rg`
  - `rg "pattern"` for basic search
  - `rg -t rust "pattern"` to search only Rust files
  - `rg -A 5 -B 5 "pattern"` for context lines
- **IDE integration**: Configure your editor for Rust development

### Testing Tools

- **Cargo test**: Built-in test runner
- **Tarpaulin**: Code coverage analysis
- **Criterion**: Benchmarking framework
- **Proptest**: Property-based testing

## Common Dependencies

### Core Libraries

- **serde**: Serialization/deserialization
- **anyhow/thiserror**: Error handling
- **log**: Structured logging
- **lexopt**: Command-line argument parsing (CLI binary only)
- **crossbeam-channel**: Thread communication

### Testing Libraries

- **proptest**: Property-based testing
- **mockall**: Mocking framework
- **criterion**: Benchmarking
- **tempfile**: Temporary file handling in tests

This guidance ensures Claude generates idiomatic, safe, and performant Rust code that follows community best practices and modern Rust development patterns.

## Project-Specific Gotchas

- **CPAL `SampleRate`**: `SampleRate` is a plain `u32` type alias, not a newtype. Never use `.0` to unwrap it — just use it directly as a `u32`.
- **`lexopt` value-swallowing**: An optional-value argument only takes the "no value" branch when it is the last token on the command line. In all other positions `lexopt` will consume the next token as the value, even if it looks like a flag.
- **Accept-Ranges and MPD**: Only emit the `Accept-Ranges` header on `206 Partial Content` and `416 Range Not Satisfiable` responses — never on `200 OK`. MPD enters an infinite GET loop if it sees `Accept-Ranges` on a 200.
- **fluent-templates bidi isolation**: Always set `set_use_isolating(false)` in the `static_loader!` macro. Without it, Fluent inserts FSI/PDI Unicode bidi-isolation characters that appear as visible garbage in FLTK labels on Windows.
