# Spanfold.Artifacts

Optional deterministic exports, explanations, audit bundles, and integrity
verification for `Spanfold` comparison results.

```csharp
using Spanfold.Artifacts;

var json = result.ExportJson();
var bundle = AuditBundleWriter.Write("artifacts/run-42", result);
var verification = AuditBundleReader.Open(bundle.Path).Verify();
```

Bundle verification proves integrity against the included manifest. It does
not authenticate who produced the bundle.
