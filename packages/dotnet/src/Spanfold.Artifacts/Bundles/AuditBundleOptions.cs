namespace Spanfold.Artifacts;

/// <summary>Configures an audit-bundle write.</summary>
public sealed record AuditBundleOptions
{
    /// <summary>Gets the default full-evidence options.</summary>
    public static AuditBundleOptions Default { get; } = new();

    /// <summary>Gets the artifact disclosure profile.</summary>
    public ArtifactExportProfile Profile { get; init; } = ArtifactExportProfile.Full;

    internal void Validate()
    {
        if (!Enum.IsDefined(Profile))
        {
            throw new ArgumentOutOfRangeException(
                nameof(Profile),
                Profile,
                "Unknown artifact export profile.");
        }
    }
}
