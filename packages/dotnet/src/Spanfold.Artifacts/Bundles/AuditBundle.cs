namespace Spanfold.Artifacts;

/// <summary>Represents an audit bundle written to or opened from disk.</summary>
public sealed record AuditBundle(string Path, AuditBundleManifest Manifest)
{
    /// <summary>Verifies every manifest file against its declared size and SHA-256 digest.</summary>
    public ArtifactVerificationResult Verify() => AuditBundleVerifier.Verify(Path, Manifest);
}
