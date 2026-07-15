using Spanfold.Internal.Analysis;
using Spanfold.Internal.Keys;
using Spanfold.Episodes;

namespace Spanfold.Internal.Episodes;

internal static class EpisodeFormationRuntime
{
    internal static EpisodeSet Run(WindowHistory history, EpisodeFormationPlan plan)
    {
        var evaluationHorizon = GetEvaluationHorizon(plan.Normalization);
        ValidateExecution(plan, evaluationHorizon);

        var windows = history.Windows;
        var fragments = new List<EpisodeFragment>(windows.Count);
        for (var i = 0; i < windows.Count; i++)
        {
            var window = windows[i];
            if (!plan.Selector.Matches(window)
                || !WindowScopeMatcher.Matches(window, plan.Scope))
            {
                continue;
            }

            EnsureHorizonClock(window, plan.Formation.TimeAxis, evaluationHorizon);
            if (!WindowRangeNormalizer.TryNormalize(
                window,
                plan.Normalization,
                plan.Normalization.KnownAt,
                out var normalized,
                out var failure))
            {
                if (failure?.Kind == WindowRangeNormalizationFailureKind.FutureWindowExcluded
                    || (failure?.Kind == WindowRangeNormalizationFailureKind.MissingEventTime
                        && plan.Normalization.NullTimestampPolicy == ComparisonNullTimestampPolicy.Exclude))
                {
                    continue;
                }

                throw new InvalidOperationException(
                    $"Window '{window.Id}' could not form an episode: {failure?.Reason ?? "Unknown normalization failure."}");
            }

            fragments.Add(new EpisodeFragment(window, normalized.Range, normalized.Finality));
        }

        fragments.Sort(CompareFragmentsForGrouping);
        var groups = GroupFragments(fragments, history.KeyComparers);
        var episodes = new List<Episode>();
        for (var i = 0; i < groups.Count; i++)
        {
            AddEpisodes(groups[i], plan, evaluationHorizon, episodes);
        }

        return new EpisodeSet(plan, episodes, evaluationHorizon, history.KeyComparers);
    }

    private static TemporalPoint? GetEvaluationHorizon(ComparisonNormalizationPolicy normalization)
    {
        return normalization.KnownAt ?? normalization.OpenWindowHorizon;
    }

    private static void ValidateExecution(
        EpisodeFormationPlan plan,
        TemporalPoint? evaluationHorizon)
    {
        if (plan.Scope.TimeAxis != plan.Formation.TimeAxis
            || plan.Normalization.TimeAxis != plan.Formation.TimeAxis)
        {
            throw new InvalidOperationException("Episode plan components must use one temporal axis.");
        }

        if (plan.Normalization.KnownAt.HasValue
            && plan.Normalization.OpenWindowHorizon.HasValue)
        {
            throw new InvalidOperationException("Episode formation accepts only one horizon source.");
        }

        if (plan.Normalization.KnownAt.HasValue
            && plan.Formation.TimeAxis == TemporalAxis.Timestamp)
        {
            throw new InvalidOperationException("Known-at episode formation is not supported on the event-time axis.");
        }

        if (evaluationHorizon.HasValue
            && evaluationHorizon.Value.Axis != plan.Formation.TimeAxis)
        {
            throw new InvalidOperationException("Episode evaluation horizon must use the plan temporal axis.");
        }
    }

    private static void EnsureHorizonClock(
        WindowRecord window,
        TemporalAxis timeAxis,
        TemporalPoint? evaluationHorizon)
    {
        if (timeAxis == TemporalAxis.Timestamp
            && evaluationHorizon.HasValue
            && window.StartTime.HasValue
            && !string.Equals(
                window.TimestampClock,
                evaluationHorizon.Value.Clock,
                StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                $"Window '{window.Id}' uses a timestamp clock incompatible with the episode horizon.");
        }
    }

    private static List<FormationGroup> GroupFragments(
        List<EpisodeFragment> fragments,
        IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers)
    {
        var groups = new List<FormationGroup>();
        for (var i = 0; i < fragments.Count; i++)
        {
            var fragment = fragments[i];
            var window = fragment.Window;
            var comparer = keyComparers.TryGetValue(window.WindowName, out var configured)
                ? configured
                : EqualityComparer<object>.Default;
            var group = FindGroup(groups, fragment, comparer);
            if (group is null)
            {
                group = new FormationGroup(
                    window.WindowName,
                    window.Key,
                    window.Source,
                    window.Partition,
                    fragment.Range.Axis,
                    fragment.Range.Start.Clock);
                groups.Add(group);
            }

            group.Fragments.Add(fragment);
        }

        return groups;
    }

    private static FormationGroup? FindGroup(
        List<FormationGroup> groups,
        EpisodeFragment fragment,
        IEqualityComparer<object> keyComparer)
    {
        var window = fragment.Window;
        for (var i = 0; i < groups.Count; i++)
        {
            var group = groups[i];
            if (string.Equals(group.WindowName, window.WindowName, StringComparison.Ordinal)
                && keyComparer.Equals(group.Key, window.Key)
                && EqualityComparer<object?>.Default.Equals(group.Source, window.Source)
                && EqualityComparer<object?>.Default.Equals(group.Partition, window.Partition)
                && group.TimeAxis == fragment.Range.Axis
                && string.Equals(group.TimestampClock, fragment.Range.Start.Clock, StringComparison.Ordinal))
            {
                return group;
            }
        }

        return null;
    }

    private static void AddEpisodes(
        FormationGroup group,
        EpisodeFormationPlan plan,
        TemporalPoint? evaluationHorizon,
        List<Episode> episodes)
    {
        group.Fragments.Sort(CompareFragmentsWithinGroup);
        var current = new List<EpisodeFragment> { group.Fragments[0] };
        var currentEnd = RequireEnd(group.Fragments[0].Range);

        for (var i = 1; i < group.Fragments.Count; i++)
        {
            var fragment = group.Fragments[i];
            if (CanStitch(currentEnd, fragment.Range.Start, plan.Formation.StitchToleranceMagnitude))
            {
                current.Add(fragment);
                var fragmentEnd = RequireEnd(fragment.Range);
                if (fragmentEnd.CompareTo(currentEnd) > 0)
                {
                    currentEnd = fragmentEnd;
                }

                continue;
            }

            episodes.Add(CreateEpisode(group, current, plan.Formation, evaluationHorizon));
            current = [fragment];
            currentEnd = RequireEnd(fragment.Range);
        }

        episodes.Add(CreateEpisode(group, current, plan.Formation, evaluationHorizon));
    }

    private static bool CanStitch(
        TemporalPoint currentEnd,
        TemporalPoint nextStart,
        long tolerance)
    {
        if (nextStart.CompareTo(currentEnd) <= 0)
        {
            return true;
        }

        var gap = TemporalMagnitudeMath.SaturatingSubtract(
            PointMagnitude(nextStart),
            PointMagnitude(currentEnd));
        return gap <= tolerance;
    }

    private static Episode CreateEpisode(
        FormationGroup group,
        List<EpisodeFragment> fragments,
        EpisodeFormationPolicy formation,
        TemporalPoint? evaluationHorizon)
    {
        var start = fragments[0].Range.Start;
        var terminal = fragments[0];
        var end = RequireEnd(terminal.Range);
        var activeMagnitude = 0L;
        var unionStart = start;
        var unionEnd = end;
        var containsProvisional = terminal.Finality == ComparisonFinality.Provisional;

        for (var i = 1; i < fragments.Count; i++)
        {
            var fragment = fragments[i];
            var fragmentEnd = RequireEnd(fragment.Range);
            containsProvisional |= fragment.Finality == ComparisonFinality.Provisional;

            if (fragment.Range.Start.CompareTo(unionEnd) > 0)
            {
                activeMagnitude = checked(activeMagnitude + RangeMagnitude(unionStart, unionEnd));
                unionStart = fragment.Range.Start;
                unionEnd = fragmentEnd;
            }
            else if (fragmentEnd.CompareTo(unionEnd) > 0)
            {
                unionEnd = fragmentEnd;
            }

            if (fragmentEnd.CompareTo(end) >= 0)
            {
                end = fragmentEnd;
                terminal = fragment;
            }
        }

        activeMagnitude = checked(activeMagnitude + RangeMagnitude(unionStart, unionEnd));
        var elapsedMagnitude = RangeMagnitude(start, end);
        if (activeMagnitude > elapsedMagnitude)
        {
            throw new InvalidOperationException("Episode active magnitude cannot exceed elapsed magnitude.");
        }

        var internalGapMagnitude = checked(elapsedMagnitude - activeMagnitude);
        var envelope = terminal.Range.EndStatus == TemporalRangeEndStatus.Closed
            ? TemporalRange.Closed(start, end)
            : TemporalRange.WithEffectiveEnd(start, end, terminal.Range.EndStatus);
        var finality = containsProvisional
            || IsWithinSettlingBoundary(end, formation.StitchToleranceMagnitude, evaluationHorizon)
                ? ComparisonFinality.Provisional
                : ComparisonFinality.Final;
        var materializedFragments = fragments.ToArray();
        var id = EpisodeIdentity.Create(
            group.WindowName,
            group.Key,
            group.Source,
            group.Partition,
            group.TimeAxis,
            materializedFragments);

        return new Episode(
            id,
            group.WindowName,
            group.Key,
            group.Source,
            group.Partition,
            envelope,
            materializedFragments,
            finality,
            activeMagnitude,
            elapsedMagnitude,
            internalGapMagnitude);
    }

    private static bool IsWithinSettlingBoundary(
        TemporalPoint lastEnd,
        long tolerance,
        TemporalPoint? evaluationHorizon)
    {
        if (!evaluationHorizon.HasValue)
        {
            return false;
        }

        var boundary = TemporalMagnitudeMath.SaturatingAdd(PointMagnitude(lastEnd), tolerance);
        return PointMagnitude(evaluationHorizon.Value) <= boundary;
    }

    private static long RangeMagnitude(TemporalPoint start, TemporalPoint end)
    {
        return TemporalMagnitudeMath.SaturatingSubtract(PointMagnitude(end), PointMagnitude(start));
    }

    private static long PointMagnitude(TemporalPoint point)
    {
        return point.Axis switch
        {
            TemporalAxis.ProcessingPosition => point.Position,
            TemporalAxis.Timestamp => point.Timestamp.UtcDateTime.Ticks,
            _ => throw new InvalidOperationException("Episode temporal points require a known axis.")
        };
    }

    private static TemporalPoint RequireEnd(TemporalRange range)
    {
        return range.End
            ?? throw new InvalidOperationException("Episode fragments require an effective end.");
    }

    private static int CompareFragmentsForGrouping(EpisodeFragment left, EpisodeFragment right)
    {
        var comparison = string.CompareOrdinal(left.Window.WindowName, right.Window.WindowName);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = CompareCanonical(left.Window.Key, right.Window.Key);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = CompareCanonical(left.Window.Source, right.Window.Source);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = CompareCanonical(left.Window.Partition, right.Window.Partition);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.Range.Axis.CompareTo(right.Range.Axis);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = string.CompareOrdinal(left.Range.Start.Clock, right.Range.Start.Clock);
        return comparison != 0 ? comparison : CompareFragmentsWithinGroup(left, right);
    }

    private static int CompareFragmentsWithinGroup(EpisodeFragment left, EpisodeFragment right)
    {
        var comparison = left.Range.Start.CompareTo(right.Range.Start);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = RequireEnd(left.Range).CompareTo(RequireEnd(right.Range));
        return comparison != 0
            ? comparison
            : string.CompareOrdinal(left.RecordId.Value, right.RecordId.Value);
    }

    private static int CompareCanonical(object? left, object? right)
    {
        return string.CompareOrdinal(
            CanonicalValueFormatter.Format(left),
            CanonicalValueFormatter.Format(right));
    }

    private sealed class FormationGroup(
        string windowName,
        object key,
        object? source,
        object? partition,
        TemporalAxis timeAxis,
        string? timestampClock)
    {
        internal string WindowName { get; } = windowName;
        internal object Key { get; } = key;
        internal object? Source { get; } = source;
        internal object? Partition { get; } = partition;
        internal TemporalAxis TimeAxis { get; } = timeAxis;
        internal string? TimestampClock { get; } = timestampClock;
        internal List<EpisodeFragment> Fragments { get; } = [];
    }
}
