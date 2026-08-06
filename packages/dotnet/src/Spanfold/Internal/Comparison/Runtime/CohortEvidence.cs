using System.Globalization;
using System.Text.Json;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Comparison;

internal sealed class CohortEvidence
{
    private readonly ComparisonSelector? cohort;
    private readonly Dictionary<WindowRecordId, object?> sourcesByRecordId;

    private CohortEvidence(
        ComparisonSelector? cohort,
        Dictionary<WindowRecordId, object?> sourcesByRecordId)
    {
        this.cohort = cohort;
        this.sourcesByRecordId = sourcesByRecordId;
    }

    internal bool HasCohort => this.cohort is not null;

    internal static CohortEvidence Create(PreparedComparison prepared)
    {
        var cohort = prepared.Plan.Against.Count == 1
            && prepared.Plan.Against[0].CohortActivity is not null
                ? prepared.Plan.Against[0]
                : (ComparisonSelector?)null;
        var sourcesByRecordId = new Dictionary<WindowRecordId, object?>();

        for (var i = 0; i < prepared.NormalizedWindows.Count; i++)
        {
            var window = prepared.NormalizedWindows[i];
            sourcesByRecordId[window.RecordId] = window.Window.Source;
        }

        return new CohortEvidence(cohort, sourcesByRecordId);
    }

    internal bool IsAgainstActive(AlignedSegment segment)
    {
        if (this.cohort is null)
        {
            return segment.AgainstRecordIds.Count > 0;
        }

        var activeSources = ActiveSources(segment);

        return this.cohort.Value.CohortActivity!.IsActive(
            activeSources.Count,
            this.cohort.Value.CohortSources.Count);
    }

    internal IReadOnlyList<ComparisonExtensionMetadata> BuildMetadata(AlignedComparison aligned)
    {
        if (!HasCohort)
        {
            return [];
        }

        var metadata = new List<ComparisonExtensionMetadata>();
        for (var i = 0; i < aligned.Segments.Count; i++)
        {
            var segment = aligned.Segments[i];
            if (segment.AgainstRecordIds.Count == 0 && segment.TargetRecordIds.Count == 0)
            {
                continue;
            }

            metadata.Add(new ComparisonExtensionMetadata(
                "spanfold.cohort",
                "segment[" + i.ToString(CultureInfo.InvariantCulture) + "]",
                Describe(segment)));
        }

        return metadata.ToArray();
    }

    private string Describe(AlignedSegment segment)
    {
        if (this.cohort is null)
        {
            return string.Empty;
        }

        var activeSources = ActiveSources(segment);
        var active = this.cohort.Value.CohortActivity!.IsActive(
            activeSources.Count,
            this.cohort.Value.CohortSources.Count);

        return JsonSerializer.Serialize(new
        {
            rule = this.cohort.Value.CohortActivity!.Name,
            required = RequiredCount(),
            activeCount = activeSources.Count,
            isActive = active,
            activeSources = activeSources.Select(static source => source?.ToString() ?? "<null>").ToArray()
        });
    }

    private int RequiredCount()
    {
        var activity = this.cohort!.Value.CohortActivity!;
        return activity.RequiredActiveCount(this.cohort.Value.CohortSources.Count);
    }

    private List<object?> ActiveSources(AlignedSegment segment)
    {
        var activeSources = new List<object?>();

        for (var i = 0; i < segment.AgainstRecordIds.Count; i++)
        {
            if (!this.sourcesByRecordId.TryGetValue(segment.AgainstRecordIds[i], out var source))
            {
                continue;
            }

            if (!ContainsSource(activeSources, source))
            {
                activeSources.Add(source);
            }
        }

        return activeSources;
    }

    private static bool ContainsSource(List<object?> sources, object? source)
    {
        for (var i = 0; i < sources.Count; i++)
        {
            if (EqualityComparer<object?>.Default.Equals(sources[i], source))
            {
                return true;
            }
        }

        return false;
    }
}
