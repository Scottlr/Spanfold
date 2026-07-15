namespace Spanfold.Artifacts;

/// <summary>Describes the versioned identity and integrity contract of an audit bundle.</summary>
public sealed record AuditBundleManifest(
    string Schema,
    int SchemaVersion,
    string Producer,
    string ProducerVersion,
    string IdentityDomain,
    ArtifactExportProfile Profile,
    string PlanFingerprint,
    string EvidenceFingerprint,
    IReadOnlyList<AuditBundleFile> Files);
