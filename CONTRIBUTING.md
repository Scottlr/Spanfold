# Contributing to Spanfold

Changes should preserve the .NET 10 contract fixtures and the Rust workspace
gates. Before opening a pull request, run the relevant checks below.

From `packages/dotnet`:

```text
dotnet restore Spanfold.slnx
dotnet test Spanfold.slnx --no-restore --configuration Release
dotnet pack src/Spanfold/Spanfold.csproj --no-restore --configuration Release --output artifacts/package
dotnet pack src/Spanfold.Testing/Spanfold.Testing.csproj --no-restore --configuration Release --output artifacts/package
dotnet pack src/Spanfold.Cli/Spanfold.Cli.csproj --no-restore --configuration Release --output artifacts/package
```

From `packages/rust`:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Keep public API changes deliberate, document compatibility impact, and add a
regression test for correctness changes. Performance claims require a checked-in
benchmark scenario and a reproducible command.
