# Package Validation

Spanfold publishes three .NET packages at one version:

1. `Spanfold`
2. `Spanfold.Artifacts`, which depends on `Spanfold`
3. `Spanfold.Testing`, which depends on both packages

`Spanfold.Cli` is checkout-only. It is built and tested with the solution but
must not produce or publish a NuGet tool package.

Run the package build:

```bash
rm -rf artifacts/package
dotnet pack src/Spanfold/Spanfold.csproj --no-restore --configuration Release --output artifacts/package
dotnet pack src/Spanfold.Artifacts/Spanfold.Artifacts.csproj --no-restore --configuration Release --output artifacts/package
dotnet pack src/Spanfold.Testing/Spanfold.Testing.csproj --no-restore --configuration Release --output artifacts/package
```

Inspect the package contents:

```bash
unzip -l artifacts/package/Spanfold.0.2.0-preview.1.nupkg
unzip -l artifacts/package/Spanfold.Artifacts.0.2.0-preview.1.nupkg
unzip -l artifacts/package/Spanfold.Testing.0.2.0-preview.1.nupkg
```

Each package contains its `net10.0` assembly, XML documentation, README, and a
matching `.snupkg`. The generated nuspec for `Spanfold.Artifacts` must depend on
`Spanfold`; the nuspec for `Spanfold.Testing` must depend on both `Spanfold` and
`Spanfold.Artifacts` at the release version.

Representative archive checks include:

```bash
unzip -Z1 artifacts/package/Spanfold.0.2.0-preview.1.nupkg | grep -Fx 'README.md'
unzip -Z1 artifacts/package/Spanfold.0.2.0-preview.1.nupkg | grep -Fx 'lib/net10.0/Spanfold.dll'
unzip -Z1 artifacts/package/Spanfold.0.2.0-preview.1.nupkg | grep -Fx 'lib/net10.0/Spanfold.xml'
unzip -Z1 artifacts/package/Spanfold.Artifacts.0.2.0-preview.1.nupkg | grep -Fx 'lib/net10.0/Spanfold.Artifacts.dll'
unzip -p artifacts/package/Spanfold.Artifacts.0.2.0-preview.1.nupkg '*.nuspec' | grep -F '<dependency id="Spanfold" version="0.2.0-preview.1"'
unzip -p artifacts/package/Spanfold.Testing.0.2.0-preview.1.nupkg '*.nuspec' | grep -F '<dependency id="Spanfold.Artifacts" version="0.2.0-preview.1"'
test ! -f artifacts/package/Spanfold.Cli.0.2.0-preview.1.nupkg
```

CI and release automation also restore each packed package into an isolated
consumer project using only `artifacts/package` as its package source. This
proves that every supported package and its transitive Spanfold dependencies
can be resolved from the release set before anything is published.

Package validation is enabled on all three publishable projects. Source Link
and symbol packages are produced for local verification; public publishing
remains a tag-triggered release step.
