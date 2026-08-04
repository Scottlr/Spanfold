# Package Validation

Spanfold package checks are designed to stay local until publish automation exists.

Run the package build:

```bash
dotnet pack src/Spanfold/Spanfold.csproj -c Release -o artifacts/package # Build the NuGet package locally.
```

Inspect the package contents:

```bash
unzip -l artifacts/package/Spanfold.0.2.0-preview.1.nupkg # Inspect library package contents.
unzip -l artifacts/package/Spanfold.0.2.0-preview.1.snupkg # Inspect symbol package contents.
```

Expected package contents include:

- `lib/net10.0/Spanfold.dll`
- `lib/net10.0/Spanfold.xml`
- `README.md`

Verify the package contract directly from the archive:

```bash
unzip -Z1 artifacts/package/Spanfold.0.2.0-preview.1.nupkg | grep -Fx 'README.md'
unzip -Z1 artifacts/package/Spanfold.0.2.0-preview.1.nupkg | grep -Fx 'lib/net10.0/Spanfold.dll'
unzip -Z1 artifacts/package/Spanfold.0.2.0-preview.1.nupkg | grep -Fx 'lib/net10.0/Spanfold.xml'
```

The same package-content and dependency checks run in CI. Package validation is
enabled in the projects, and Source Link and symbol packages are produced for
local verification while public publishing remains a separate release step.
