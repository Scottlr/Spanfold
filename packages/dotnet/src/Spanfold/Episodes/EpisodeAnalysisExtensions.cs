using Spanfold.Internal.Episodes;

namespace Spanfold.Episodes;

/// <summary>
/// Provides neutral relation queries and explicit reference interpretation.
/// </summary>
public static class EpisodeAnalysisExtensions
{
    /// <summary>
    /// Returns relation components of one kind in deterministic result order.
    /// </summary>
    /// <param name="result">The episode-comparison result.</param>
    /// <param name="kind">The component kind to select.</param>
    /// <returns>A new read-only relation collection.</returns>
    public static IReadOnlyList<EpisodeRelation> RelationsOfKind(
        this EpisodeComparisonResult result,
        EpisodeRelationKind kind)
    {
        ArgumentNullException.ThrowIfNull(result);
        var matches = new List<EpisodeRelation>();
        for (var i = 0; i < result.Relations.Count; i++)
        {
            if (result.Relations[i].Kind == kind)
            {
                matches.Add(result.Relations[i]);
            }
        }

        return Array.AsReadOnly(matches.ToArray());
    }

    /// <summary>
    /// Returns target episodes with no against relationship.
    /// </summary>
    /// <param name="result">The episode-comparison result.</param>
    /// <returns>A new read-only episode collection.</returns>
    public static IReadOnlyList<Episode> UnmatchedTargetEpisodes(
        this EpisodeComparisonResult result)
    {
        return UnmatchedEpisodes(result, EpisodeRelationKind.UnmatchedTarget, target: true);
    }

    /// <summary>
    /// Returns against episodes with no target relationship.
    /// </summary>
    /// <param name="result">The episode-comparison result.</param>
    /// <returns>A new read-only episode collection.</returns>
    public static IReadOnlyList<Episode> UnmatchedAgainstEpisodes(
        this EpisodeComparisonResult result)
    {
        return UnmatchedEpisodes(result, EpisodeRelationKind.UnmatchedAgainst, target: false);
    }

    /// <summary>
    /// Explicitly interprets target episodes as references and against episodes as detections.
    /// </summary>
    /// <param name="result">The neutral episode-comparison result.</param>
    /// <returns>A directional reference scorecard.</returns>
    public static EpisodeReferenceScorecard AsReference(this EpisodeComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        return EpisodeSummaryRuntime.AsReference(result);
    }

    private static IReadOnlyList<Episode> UnmatchedEpisodes(
        EpisodeComparisonResult result,
        EpisodeRelationKind kind,
        bool target)
    {
        ArgumentNullException.ThrowIfNull(result);
        var episodes = new List<Episode>();
        for (var i = 0; i < result.Relations.Count; i++)
        {
            var relation = result.Relations[i];
            if (relation.Kind != kind)
            {
                continue;
            }

            episodes.AddRange(target ? relation.TargetEpisodes : relation.AgainstEpisodes);
        }

        return Array.AsReadOnly(episodes.ToArray());
    }
}
