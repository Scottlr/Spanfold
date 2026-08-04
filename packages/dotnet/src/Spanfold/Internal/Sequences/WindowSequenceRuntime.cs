using Spanfold.Internal.Keys;
using Spanfold.Sequences;

namespace Spanfold.Internal.Sequences;

internal static class WindowSequenceRuntime
{
    internal static WindowSequenceResult Run(
        WindowHistory history,
        WindowSequencePlan plan,
        TemporalPoint? evaluationHorizon)
    {
        var evidence = evaluationHorizon.HasValue
            ? MaterializeLiveEvidence(history, plan, evaluationHorizon.Value)
            : MaterializeHistoricalEvidence(history, plan);
        var keyComparer = history.KeyComparers.TryGetValue(plan.Steps[0], out var configured)
            ? configured
            : EqualityComparer<object>.Default;
        var groups = GroupByCorrelationLane(evidence, plan, keyComparer);
        var matches = new List<WindowSequenceMatch>();

        for (var i = 0; i < groups.Count; i++)
        {
            MatchGroup(groups[i], plan, matches);
        }

        matches.Sort(CompareMatches);
        return new WindowSequenceResult(plan, matches, evaluationHorizon);
    }

    private static List<SequenceEvidence> MaterializeHistoricalEvidence(
        WindowHistory history,
        WindowSequencePlan plan)
    {
        var selectedFamilies = plan.Steps.ToHashSet(StringComparer.Ordinal);
        var windows = history.Windows;
        var evidence = new List<SequenceEvidence>();

        for (var i = 0; i < windows.Count; i++)
        {
            var window = windows[i];
            if (!selectedFamilies.Contains(window.WindowName))
            {
                continue;
            }

            if (window is not ClosedWindow closed)
            {
                throw new InvalidOperationException(
                    $"Historical sequence '{plan.Name}' cannot use open window '{window.Id}'. Use RunLive with an explicit horizon.");
            }

            var range = TemporalRange.Closed(
                TemporalPoint.ForPosition(closed.StartPosition),
                TemporalPoint.ForPosition(closed.EndPosition!.Value));
            evidence.Add(new SequenceEvidence(
                new WindowSnapshotRecord(closed, range, ComparisonFinality.Final)));
        }

        evidence.Sort(CompareEvidence);
        return evidence;
    }

    private static List<SequenceEvidence> MaterializeLiveEvidence(
        WindowHistory history,
        WindowSequencePlan plan,
        TemporalPoint evaluationHorizon)
    {
        var selectedFamilies = plan.Steps.ToHashSet(StringComparer.Ordinal);
        var records = history.SnapshotAt(evaluationHorizon).Records;
        var evidence = new List<SequenceEvidence>(records.Count);

        for (var i = 0; i < records.Count; i++)
        {
            if (selectedFamilies.Contains(records[i].Window.WindowName))
            {
                evidence.Add(new SequenceEvidence(records[i]));
            }
        }

        evidence.Sort(CompareEvidence);
        return evidence;
    }

    private static List<SequenceLane> GroupByCorrelationLane(
        List<SequenceEvidence> evidence,
        WindowSequencePlan plan,
        IEqualityComparer<object> keyComparer)
    {
        var firstFamily = plan.Steps[0];
        var groups = new List<SequenceLane>();

        for (var i = 0; i < evidence.Count; i++)
        {
            var candidate = evidence[i];
            if (!string.Equals(candidate.Window.WindowName, firstFamily, StringComparison.Ordinal)
                || FindLane(groups, candidate, keyComparer) is not null)
            {
                continue;
            }

            groups.Add(new SequenceLane(
                candidate.Window.Key,
                candidate.Window.Source,
                candidate.Window.Partition));
        }

        for (var i = 0; i < evidence.Count; i++)
        {
            var candidate = evidence[i];
            var group = FindLane(groups, candidate, keyComparer);
            group?.Add(candidate);
        }

        return groups;
    }

    private static SequenceLane? FindLane(
        List<SequenceLane> groups,
        SequenceEvidence candidate,
        IEqualityComparer<object> keyComparer)
    {
        for (var i = 0; i < groups.Count; i++)
        {
            var group = groups[i];
            if (keyComparer.Equals(group.Key, candidate.Window.Key)
                && EqualityComparer<object?>.Default.Equals(group.Source, candidate.Window.Source)
                && EqualityComparer<object?>.Default.Equals(group.Partition, candidate.Window.Partition))
            {
                return group;
            }
        }

        return null;
    }

    private static void MatchGroup(
        SequenceLane group,
        WindowSequencePlan plan,
        List<WindowSequenceMatch> matches)
    {
        if (!group.TryGetCandidates(plan.Steps[0], out var firstCandidates))
        {
            return;
        }

        var used = new HashSet<WindowRecordId>();
        for (var i = 0; i < firstCandidates.Count; i++)
        {
            var first = firstCandidates[i];
            if (used.Contains(first.Window.Id))
            {
                continue;
            }

            var chain = TryBuildChain(group, plan, first, used);
            if (chain is null)
            {
                continue;
            }

            for (var stepIndex = 0; stepIndex < chain.Count; stepIndex++)
            {
                used.Add(chain[stepIndex].Window.Id);
            }

            matches.Add(CreateMatch(group, chain));
        }
    }

    private static List<SequenceEvidence>? TryBuildChain(
        SequenceLane group,
        WindowSequencePlan plan,
        SequenceEvidence first,
        HashSet<WindowRecordId> used)
    {
        var selected = new HashSet<WindowRecordId> { first.Window.Id };
        var chain = new List<SequenceEvidence>(plan.Steps.Count) { first };
        var previous = first;

        for (var stepIndex = 1; stepIndex < plan.Steps.Count; stepIndex++)
        {
            if (!group.TryGetCandidates(plan.Steps[stepIndex], out var candidates))
            {
                return null;
            }

            var next = FindNext(candidates, previous, plan.MaximumGap, used, selected);
            if (next is null)
            {
                return null;
            }

            chain.Add(next);
            selected.Add(next.Window.Id);
            previous = next;
        }

        return chain;
    }

    private static SequenceEvidence? FindNext(
        List<SequenceEvidence> candidates,
        SequenceEvidence previous,
        long? maximumGap,
        HashSet<WindowRecordId> used,
        HashSet<WindowRecordId> selected)
    {
        for (var i = 0; i < candidates.Count; i++)
        {
            var candidate = candidates[i];
            if (used.Contains(candidate.Window.Id)
                || selected.Contains(candidate.Window.Id)
                || candidate.Start < previous.Start)
            {
                continue;
            }

            var gap = Math.Max(0, candidate.Start - previous.End);
            if (!maximumGap.HasValue || gap <= maximumGap.Value)
            {
                return candidate;
            }
        }

        return null;
    }

    private static WindowSequenceMatch CreateMatch(
        SequenceLane group,
        List<SequenceEvidence> chain)
    {
        var end = chain[0].End;
        var totalGap = 0L;
        var finality = ComparisonFinality.Final;
        var records = new WindowSnapshotRecord[chain.Count];

        for (var i = 0; i < chain.Count; i++)
        {
            var item = chain[i];
            records[i] = item.Record;
            end = Math.Max(end, item.End);
            if (item.Record.Finality != ComparisonFinality.Final)
            {
                finality = ComparisonFinality.Provisional;
            }

            if (i > 0)
            {
                totalGap = checked(totalGap + Math.Max(0, item.Start - chain[i - 1].End));
            }
        }

        return new WindowSequenceMatch(
            group.Key,
            group.Source,
            group.Partition,
            records,
            chain[0].Start,
            end,
            totalGap,
            finality);
    }

    private static int CompareMatches(WindowSequenceMatch left, WindowSequenceMatch right)
    {
        var comparison = left.EndPosition.CompareTo(right.EndPosition);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.StartPosition.CompareTo(right.StartPosition);
        if (comparison != 0)
        {
            return comparison;
        }

        for (var i = 0; i < left.Evidence.Count; i++)
        {
            comparison = string.Compare(
                left.Evidence[i].Window.Id.Value,
                right.Evidence[i].Window.Id.Value,
                StringComparison.Ordinal);
            if (comparison != 0)
            {
                return comparison;
            }
        }

        return 0;
    }

    private static int CompareEvidence(SequenceEvidence left, SequenceEvidence right)
    {
        var comparison = left.End.CompareTo(right.End);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.Start.CompareTo(right.Start);
        return comparison != 0
            ? comparison
            : string.Compare(left.Window.Id.Value, right.Window.Id.Value, StringComparison.Ordinal);
    }

    private sealed class SequenceLane(object key, object? source, object? partition)
    {
        private readonly Dictionary<string, List<SequenceEvidence>> candidates = new(StringComparer.Ordinal);

        internal object Key { get; } = key;
        internal object? Source { get; } = source;
        internal object? Partition { get; } = partition;

        internal void Add(SequenceEvidence evidence)
        {
            if (!this.candidates.TryGetValue(evidence.Window.WindowName, out var values))
            {
                values = [];
                this.candidates.Add(evidence.Window.WindowName, values);
            }

            values.Add(evidence);
        }

        internal bool TryGetCandidates(string windowName, out List<SequenceEvidence> values)
        {
            return this.candidates.TryGetValue(windowName, out values!);
        }
    }

    private sealed class SequenceEvidence(WindowSnapshotRecord record)
    {
        internal WindowSnapshotRecord Record { get; } = record;
        internal WindowRecord Window => this.Record.Window;
        internal long Start => this.Record.Range.Start.Position;
        internal long End => this.Record.Range.End!.Value.Position;
    }
}
