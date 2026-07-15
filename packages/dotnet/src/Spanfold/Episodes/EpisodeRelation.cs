namespace Spanfold.Episodes;

/// <summary>
/// Represents one connected component in the target-against episode graph.
/// </summary>
public sealed record EpisodeRelation
{
    internal EpisodeRelation(
        EpisodeRelationKind kind,
        IReadOnlyList<Episode> targetEpisodes,
        IReadOnlyList<Episode> againstEpisodes,
        EpisodeRelationMetrics metrics,
        ComparisonFinality finality)
    {
        Kind = kind;
        TargetEpisodes = Array.AsReadOnly(targetEpisodes.ToArray());
        AgainstEpisodes = Array.AsReadOnly(againstEpisodes.ToArray());
        Metrics = metrics;
        Finality = finality;
    }

    /// <summary>Gets the directional component classification.</summary>
    public EpisodeRelationKind Kind { get; }

    /// <summary>Gets the deterministically ordered target episodes.</summary>
    public IReadOnlyList<Episode> TargetEpisodes { get; }

    /// <summary>Gets the deterministically ordered against episodes.</summary>
    public IReadOnlyList<Episode> AgainstEpisodes { get; }

    /// <summary>Gets the component-level active and timing metrics.</summary>
    public EpisodeRelationMetrics Metrics { get; }

    /// <summary>Gets whether the relation can still change at its evaluation horizon.</summary>
    public ComparisonFinality Finality { get; }
}
