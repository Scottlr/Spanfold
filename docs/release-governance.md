# Release and schema governance

The .NET library, testing package, and CLI are released from the same source
revision and use the `VersionPrefix` in `packages/dotnet/Directory.Build.props`.
Do not add a project-local version. A release changes that one value, updates
the changelog, and publishes all related packages together.

Exported comparison artifacts currently use schema version `0`. Fixture files
used by the CLI use schema version `1`. A schema version is a compatibility
contract: additive fields are allowed within a version, while renamed,
removed, or retyped fields require a new version and a documented migration
path. Readers must reject unsupported versions rather than silently guessing.

Preview packages may change schemas between minor releases, but every such
change must be recorded in the changelog and covered by the package validation
workflow before publishing.
