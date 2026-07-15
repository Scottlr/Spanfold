namespace Spanfold.Episodes;

/// <summary>
/// Contains two formed episode sets and their exhaustive relation components.
/// </summary>
public sealed record EpisodeComparisonResult
{
    internal EpisodeComparisonResult(
        EpisodeComparisonPlan plan,
        EpisodeSet targetEpisodes,
        EpisodeSet againstEpisodes,
        IReadOnlyList<EpisodeRelation> relations,
        EpisodeComparisonSummary summary,
        TemporalPoint? evaluationHorizon)
    {
        Plan = plan;
        TargetEpisodes = targetEpisodes;
        AgainstEpisodes = againstEpisodes;
        Relations = Array.AsReadOnly(relations.ToArray());
        Summary = summary;
        EvaluationHorizon = evaluationHorizon;
    }

    /// <summary>Gets the analytical comparison name.</summary>
    public string Name => Plan.Name;

    /// <summary>Gets the effective plan used by both sides.</summary>
    public EpisodeComparisonPlan Plan { get; }

    /// <summary>Gets the formed target episodes.</summary>
    public EpisodeSet TargetEpisodes { get; }

    /// <summary>Gets the formed against episodes.</summary>
    public EpisodeSet AgainstEpisodes { get; }

    /// <summary>Gets the deterministic, exhaustive relation components.</summary>
    public IReadOnlyList<EpisodeRelation> Relations { get; }

    /// <summary>Gets the materialized neutral relationship summary.</summary>
    public EpisodeComparisonSummary Summary { get; }

    /// <summary>Gets the live or known-at evaluation horizon, when present.</summary>
    public TemporalPoint? EvaluationHorizon { get; }
}
