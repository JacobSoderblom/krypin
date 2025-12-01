# Agent Guidelines

- Always run the same checks as the CI pipeline before submitting changes:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace --all-targets`

## Rust Best Practices
- Prefer small, focused modules and functions with clear ownership semantics.
- Derive common traits (e.g., `Debug`, `Clone`, `Copy`, `Eq`, `PartialEq`, `Default`) when it improves ergonomics and readability.
- Use `?` for error propagation and return structured error types instead of panicking in library code.
- Avoid unnecessary `unwrap`/`expect`; if they are required, add context.
- Keep imports organized and avoid wildcard imports unless justified.
- Document public APIs with `///` doc comments, including examples when helpful.
- Favor iterator adapters over manual loops when they improve clarity.
- Use `clippy::pedantic` lint suggestions when practical, and silence lints sparingly with `#[allow]` plus justification.
- Keep `match` statements exhaustive, preferring `unreachable!`/`todo!` only with clear rationale.
