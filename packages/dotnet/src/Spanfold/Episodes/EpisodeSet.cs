using System.Collections.ObjectModel;

namespace Spanfold.Episodes;

/// <summary>
/// Contains the materialized episodes produced by one formation plan.
/// </summary>
public sealed record EpisodeSet
{
    internal EpisodeSet(
        EpisodeFormationPlan plan,
        IReadOnlyList<Episode> episodes,
        TemporalPoint? evaluationHorizon,
        IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers)
    {
        Plan = plan;
        Episodes = Array.AsReadOnly(episodes.ToArray());
        EvaluationHorizon = evaluationHorizon;
        KeyComparers = CopyComparers(keyComparers);
    }

    /// <summary>Gets the analytical episode-set name.</summary>
    public string Name => Plan.Name;

    /// <summary>Gets the effective plan used to form this result.</summary>
    public EpisodeFormationPlan Plan { get; }

    /// <summary>Gets the deterministically ordered episodes.</summary>
    public IReadOnlyList<Episode> Episodes { get; }

    /// <summary>Gets the live or known-at evaluation horizon, when present.</summary>
    public TemporalPoint? EvaluationHorizon { get; }

    internal IReadOnlyDictionary<string, IEqualityComparer<object>> KeyComparers { get; }

    private static IReadOnlyDictionary<string, IEqualityComparer<object>> CopyComparers(
        IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers)
    {
        var copy = new Dictionary<string, IEqualityComparer<object>>(StringComparer.Ordinal);
        foreach (var pair in keyComparers)
        {
            copy.Add(pair.Key, pair.Value);
        }

        return new ReadOnlyDictionary<string, IEqualityComparer<object>>(copy);
    }
}
