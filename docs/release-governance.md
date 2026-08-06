# Release and schema governance

The supported .NET package graph is:

1. `Spanfold`
2. `Spanfold.Artifacts` → `Spanfold`
3. `Spanfold.Testing` → `Spanfold.Artifacts` and `Spanfold`

All three packages are released from the same source revision and use the
`Version` in `packages/dotnet/Directory.Build.props`. Do not add a project-local
version. A release changes that one value, updates the changelog, packs and
validates the complete set, then publishes it in the dependency order above.

`Spanfold.Cli` is a checkout-only composition shell. It remains in the solution
so its workflows are built and tested, but it is not packed or published as a
NuGet tool. Changing that distribution boundary is a separate product and
release-governance decision.

Exported comparison artifacts currently use schema version `0`. Fixture files
used by the CLI use schema version `1`. A schema version is a compatibility
contract: additive fields are allowed within a version, while renamed,
removed, or retyped fields require a new version and a documented migration
path. Readers must reject unsupported versions rather than silently guessing.

Preview packages may change schemas between minor releases, but every such
change must be recorded in the changelog and covered by the package validation
workflow before publishing. A release must not proceed unless generated nuspec
dependencies and isolated restores succeed for every supported package.
