# Contributing to Spanfold

Changes should preserve the .NET 10 contract fixtures and the Rust workspace
gates. Before opening a pull request, run from `packages/rust`:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Keep public API changes deliberate, document compatibility impact, and add a
regression test for correctness changes. Performance claims require a checked-in
benchmark scenario and a reproducible command.
