# Spanfold.Artifacts

Contract fixture execution, deterministic exports, explanations, audit bundles,
and integrity verification for `Spanfold` comparison results.

Install `Spanfold.Artifacts` at the same version as `Spanfold`. The package has
a direct dependency on the matching core package and is published before
`Spanfold.Testing`, which depends on both.

```csharp
using Spanfold.Artifacts;

var json = result.ExportJson();
var bundle = AuditBundleWriter.Write("artifacts/run-42", result);
var verification = AuditBundleReader.Open(bundle.Path).Verify();
```

`Spanfold.Artifacts.Comparison.ComparisonFixtureRunner.Run(...)` is the owning
API for validating, constructing, and executing schema-version-1 contract
fixtures. The legacy `Spanfold.Testing.ContractFixtureRunner` API delegates to
this implementation.

Bundle verification proves integrity against the included manifest. It does
not authenticate who produced the bundle.
