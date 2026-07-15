namespace Spanfold.Artifacts;

/// <summary>Represents audit-bundle integrity verification.</summary>
public sealed class ArtifactVerificationResult
{
    internal ArtifactVerificationResult(IEnumerable<string> errors)
    {
        Errors = Array.AsReadOnly(errors.ToArray());
    }

    /// <summary>Gets whether every declared bundle file matched the manifest.</summary>
    public bool IsValid => Errors.Count == 0;

    /// <summary>Gets deterministic verification failures.</summary>
    public IReadOnlyList<string> Errors { get; }
}
