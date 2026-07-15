namespace Spanfold.Artifacts;

/// <summary>Controls whether an audit bundle contains full or value-redacted evidence.</summary>
public enum ArtifactExportProfile
{
    /// <summary>Includes the full comparison result and optional supporting artifacts.</summary>
    Full,

    /// <summary>Includes only counts, row identities, finality, and diagnostic codes.</summary>
    Redacted
}
