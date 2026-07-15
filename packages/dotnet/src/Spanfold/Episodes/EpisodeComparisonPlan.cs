namespace Spanfold.Episodes;

/// <summary>
/// Describes two episode definitions and the policy used to relate them.
/// </summary>
public sealed record EpisodeComparisonPlan
{
    internal EpisodeComparisonPlan(
        string name,
        string targetName,
        ComparisonSelector target,
        string againstName,
        ComparisonSelector against,
        ComparisonScope scope,
        ComparisonNormalizationPolicy normalization,
        EpisodeFormationPolicy formation,
        EpisodeRelationPolicy relation)
    {
        Name = name;
        TargetName = targetName;
        Target = target;
        AgainstName = againstName;
        Against = against;
        Scope = scope;
        Normalization = normalization;
        Formation = formation;
        Relation = relation;
    }

    /// <summary>Gets the analytical comparison name.</summary>
    public string Name { get; }

    /// <summary>Gets the display name for target episodes.</summary>
    public string TargetName { get; }

    /// <summary>Gets the target window selector.</summary>
    public ComparisonSelector Target { get; }

    /// <summary>Gets the display name for against episodes.</summary>
    public string AgainstName { get; }

    /// <summary>Gets the against window selector.</summary>
    public ComparisonSelector Against { get; }

    /// <summary>Gets the named window scope shared by both sides.</summary>
    public ComparisonScope Scope { get; }

    /// <summary>Gets the normalization policy shared by both sides.</summary>
    public ComparisonNormalizationPolicy Normalization { get; }

    /// <summary>Gets the episode-formation policy shared by both sides.</summary>
    public EpisodeFormationPolicy Formation { get; }

    /// <summary>Gets the cross-side relation policy.</summary>
    public EpisodeRelationPolicy Relation { get; }
}
