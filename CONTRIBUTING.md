# Contributing to Spanfold

Changes should preserve the .NET 10 contract fixtures and the Rust workspace
gates. Before opening a pull request, run the relevant checks below.

From `packages/dotnet`:

```text
dotnet restore Spanfold.slnx
dotnet format Spanfold.slnx --no-restore --verify-no-changes --severity warn
dotnet build Spanfold.slnx --no-restore --configuration Release
dotnet test Spanfold.slnx --no-restore --no-build --configuration Release
dotnet pack src/Spanfold/Spanfold.csproj --no-restore --configuration Release --output artifacts/package
dotnet pack src/Spanfold.Artifacts/Spanfold.Artifacts.csproj --no-restore --configuration Release --output artifacts/package
dotnet pack src/Spanfold.Testing/Spanfold.Testing.csproj --no-restore --configuration Release --output artifacts/package
```

The repository-root `.editorconfig` owns .NET formatting, code style, naming,
and per-rule analyzer severity. `Directory.Build.props` enables the first-party
.NET SDK analyzers during builds; no separate analyzer command or package is
required.

The published package order is `Spanfold`, `Spanfold.Artifacts`, then
`Spanfold.Testing`. `Spanfold.Cli` is built and tested from the solution but is
not packed or published as a NuGet tool. Follow
[`docs/package-validation.md`](docs/package-validation.md) when changing the
package graph or metadata.

From `packages/rust`:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Keep public API changes deliberate, document compatibility impact, and add a
regression test for correctness changes. Performance claims require a checked-in
benchmark scenario and a reproducible command.
