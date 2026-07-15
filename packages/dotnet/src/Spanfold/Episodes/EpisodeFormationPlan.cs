namespace Spanfold.Episodes;

/// <summary>
/// Describes selection, normalization, and stitching for one episode set.
/// </summary>
public sealed record EpisodeFormationPlan
{
    internal EpisodeFormationPlan(
        string name,
        ComparisonSelector selector,
        ComparisonScope scope,
        ComparisonNormalizationPolicy normalization,
        EpisodeFormationPolicy formation)
    {
        Name = name;
        Selector = selector;
        Scope = scope;
        Normalization = normalization;
        Formation = formation;
    }

    /// <summary>Gets the analytical name for the episode set.</summary>
    public string Name { get; }

    /// <summary>Gets the source-window selector.</summary>
    public ComparisonSelector Selector { get; }

    /// <summary>Gets the named window scope and temporal axis.</summary>
    public ComparisonScope Scope { get; }

    /// <summary>Gets the window normalization policy.</summary>
    public ComparisonNormalizationPolicy Normalization { get; }

    /// <summary>Gets the fragment stitching policy.</summary>
    public EpisodeFormationPolicy Formation { get; }
}
