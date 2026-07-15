namespace Spanfold.Artifacts;

/// <summary>Describes canonical row metadata parsed from a comparison artifact.</summary>
public sealed record ComparisonArtifactRow(
    ComparisonRowReference Reference,
    ComparisonFinality Finality,
    int Version);
