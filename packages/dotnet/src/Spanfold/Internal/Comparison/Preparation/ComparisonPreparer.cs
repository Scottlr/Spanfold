using System.Globalization;

using Spanfold;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Comparison;

internal static class ComparisonPreparer
{
    internal static PreparedComparison Prepare(WindowHistory history, ComparisonPlan plan)
    {
        var diagnostics = new List<ComparisonPlanDiagnostic>(plan.Validate());
        var selected = new List<WindowRecord>();
        var excluded = new List<ExcludedWindowRecord>();
        var normalized = new List<NormalizedWindowRecord>();
        var memberships = new HashSet<(WindowRecordId RecordId, ComparisonSide Side)>();

        if (plan.Target is null || plan.Scope is null)
        {
            return Create(plan, diagnostics, selected, excluded, normalized);
        }

        if (plan.Scope.TimeAxis != plan.Normalization.TimeAxis)
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.MixedTimeAxes,
                "Comparison scope and normalization policy use different temporal axes.",
                "normalization.timeAxis",
                ComparisonPlanDiagnosticSeverity.Error));
        }

        var knownAt = plan.Normalization.KnownAt;
        var knownAtFilter = default(TemporalPoint);
        var canFilterByKnownAt = false;

        if (knownAt.HasValue
            && knownAt.Value.Axis != TemporalAxis.ProcessingPosition)
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.KnownAtRequiresProcessingPosition,
                "Known-at filtering currently requires processing-position availability information.",
                "normalization.knownAt",
                ComparisonPlanDiagnosticSeverity.Error));
        }
        else if (knownAt.HasValue)
        {
            knownAtFilter = knownAt.Value;
            canFilterByKnownAt = true;
        }

        var windows = history.Windows
            .OrderBy(static window => window.WindowName, StringComparer.Ordinal)
            .ThenBy(static window => StableObjectValue(window.Key), StringComparer.Ordinal)
            .ThenBy(static window => StableObjectValue(window.Source), StringComparer.Ordinal)
            .ThenBy(static window => StableObjectValue(window.Partition), StringComparer.Ordinal)
            .ThenBy(static window => window.StartPosition)
            .ThenBy(static window => window.EndPosition ?? long.MaxValue)
            .ToArray();

        foreach (var window in windows)
        {
            if (canFilterByKnownAt && !IsKnownAt(window, knownAtFilter))
            {
                AddExclusion(
                    window,
                    "Window was not available at the configured known-at point.",
                    ComparisonPlanValidationCode.FutureWindowExcluded,
                    diagnostics,
                    excluded,
                    ComparisonPlanDiagnosticSeverity.Warning);
                continue;
            }

            if (!IsInScope(window, plan.Scope))
            {
                excluded.Add(new ExcludedWindowRecord(window, "Window is outside the comparison scope."));
                continue;
            }

            var matched = false;
            if (plan.Target.Value.Matches(window))
            {
                matched = true;
                AddNormalized(window, plan.Target.Value.Name, ComparisonSide.Target, plan, diagnostics, selected, excluded, normalized, memberships);
            }

            for (var i = 0; i < plan.Against.Count; i++)
            {
                var selector = plan.Against[i];
                if (!selector.Matches(window))
                {
                    continue;
                }

                matched = true;
                AddNormalized(window, selector.Name, ComparisonSide.Against, plan, diagnostics, selected, excluded, normalized, memberships);
            }

            if (!matched)
            {
                excluded.Add(new ExcludedWindowRecord(window, "Window did not match target or comparison selectors."));
            }
        }

        return Create(plan, diagnostics, selected, excluded, normalized);
    }

    private static void AddNormalized(
        WindowRecord window,
        string selectorName,
        ComparisonSide side,
        ComparisonPlan plan,
        List<ComparisonPlanDiagnostic> diagnostics,
        List<WindowRecord> selected,
        List<ExcludedWindowRecord> excluded,
        List<NormalizedWindowRecord> normalized,
        HashSet<(WindowRecordId RecordId, ComparisonSide Side)> memberships)
    {
        if (!memberships.Add((window.Id, side)))
        {
            return;
        }

        if (!TryCreateRange(window, plan.Normalization, diagnostics, excluded, out var range))
        {
            return;
        }

        if (!selected.Contains(window))
        {
            selected.Add(window);
        }

        normalized.Add(new NormalizedWindowRecord(window, window.Id, selectorName, side, range, window.Segments));
    }

    private static bool TryCreateRange(
        WindowRecord window,
        ComparisonNormalizationPolicy policy,
        List<ComparisonPlanDiagnostic> diagnostics,
        List<ExcludedWindowRecord> excluded,
        out TemporalRange range)
    {
        range = default;

        if (policy.TimeAxis == TemporalAxis.Timestamp)
        {
            return TryCreateTimestampRange(window, policy, diagnostics, excluded, out range);
        }

        var start = TemporalPoint.ForPosition(window.StartPosition);
        if (window.EndPosition.HasValue)
        {
            range = TemporalRange.Closed(start, TemporalPoint.ForPosition(window.EndPosition.Value));
            return true;
        }

        if (policy.OpenWindowPolicy == ComparisonOpenWindowPolicy.ClipToHorizon
            && policy.OpenWindowHorizon.HasValue)
        {
            return TryCreateClippedRange(
                window,
                start,
                policy.OpenWindowHorizon.Value,
                diagnostics,
                excluded,
                out range);
        }

        AddExclusion(window, "Open windows require an explicit clipping policy.", ComparisonPlanValidationCode.OpenWindowsWithoutPolicy, diagnostics, excluded);
        return false;
    }

    private static bool TryCreateTimestampRange(
        WindowRecord window,
        ComparisonNormalizationPolicy policy,
        List<ComparisonPlanDiagnostic> diagnostics,
        List<ExcludedWindowRecord> excluded,
        out TemporalRange range)
    {
        range = default;

        if (!window.StartTime.HasValue || (!window.EndTime.HasValue && window.IsClosed))
        {
            AddExclusion(
                window,
                "Event-time comparison requires recorded event timestamps.",
                ComparisonPlanValidationCode.MissingEventTime,
                diagnostics,
                excluded,
                policy.NullTimestampPolicy == ComparisonNullTimestampPolicy.Reject
                    ? ComparisonPlanDiagnosticSeverity.Error
                    : ComparisonPlanDiagnosticSeverity.Warning);
            return false;
        }

        var start = TemporalPoint.ForTimestamp(window.StartTime.Value);
        if (window.EndTime.HasValue)
        {
            range = TemporalRange.Closed(start, TemporalPoint.ForTimestamp(window.EndTime.Value));
            return true;
        }

        if (policy.OpenWindowPolicy == ComparisonOpenWindowPolicy.ClipToHorizon
            && policy.OpenWindowHorizon.HasValue)
        {
            return TryCreateClippedRange(
                window,
                start,
                policy.OpenWindowHorizon.Value,
                diagnostics,
                excluded,
                out range);
        }

        AddExclusion(window, "Open windows require an explicit clipping policy.", ComparisonPlanValidationCode.OpenWindowsWithoutPolicy, diagnostics, excluded);
        return false;
    }

    private static bool TryCreateClippedRange(
        WindowRecord window,
        TemporalPoint start,
        TemporalPoint horizon,
        List<ComparisonPlanDiagnostic> diagnostics,
        List<ExcludedWindowRecord> excluded,
        out TemporalRange range)
    {
        range = default;

        if (horizon.Axis != start.Axis)
        {
            AddExclusion(
                window,
                "Open-window horizon must use the same temporal axis as the normalized range.",
                ComparisonPlanValidationCode.MixedTimeAxes,
                diagnostics,
                excluded);
            return false;
        }

        if (horizon.CompareTo(start) < 0)
        {
            AddExclusion(
                window,
                "Open-window horizon cannot be earlier than the window start.",
                ComparisonPlanValidationCode.InvalidRangeDuration,
                diagnostics,
                excluded);
            return false;
        }

        range = TemporalRange.WithEffectiveEnd(start, horizon, TemporalRangeEndStatus.OpenAtHorizon);
        return true;
    }

    private static void AddExclusion(
        WindowRecord window,
        string reason,
        ComparisonPlanValidationCode code,
        List<ComparisonPlanDiagnostic> diagnostics,
        List<ExcludedWindowRecord> excluded,
        ComparisonPlanDiagnosticSeverity severity = ComparisonPlanDiagnosticSeverity.Error)
    {
        excluded.Add(new ExcludedWindowRecord(window, reason, code));
        diagnostics.Add(new ComparisonPlanDiagnostic(code, reason, $"window[{window.Id}]", severity));
    }

    private static bool IsInScope(WindowRecord window, ComparisonScope scope)
    {
        if (scope.WindowName is not null
            && !string.Equals(window.WindowName, scope.WindowName, StringComparison.Ordinal))
        {
            return false;
        }

        for (var i = 0; i < scope.SegmentFilters.Count; i++)
        {
            var filter = scope.SegmentFilters[i];
            if (!HasSegment(window, filter))
            {
                return false;
            }
        }

        for (var i = 0; i < scope.TagFilters.Count; i++)
        {
            var filter = scope.TagFilters[i];
            if (!HasTag(window, filter))
            {
                return false;
            }
        }

        return true;
    }

    private static bool HasSegment(WindowRecord window, WindowSegmentFilter filter)
    {
        for (var i = 0; i < window.Segments.Count; i++)
        {
            var segment = window.Segments[i];
            if (string.Equals(segment.Name, filter.Name, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(segment.Value, filter.Value))
            {
                return true;
            }
        }

        return false;
    }

    private static bool HasTag(WindowRecord window, WindowTagFilter filter)
    {
        for (var i = 0; i < window.Tags.Count; i++)
        {
            var tag = window.Tags[i];
            if (string.Equals(tag.Name, filter.Name, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(tag.Value, filter.Value))
            {
                return true;
            }
        }

        return false;
    }

    private static bool IsKnownAt(WindowRecord window, TemporalPoint knownAt)
    {
        var availabilityPosition = window.EndPosition ?? window.StartPosition;
        return availabilityPosition <= knownAt.Position;
    }

    private static PreparedComparison Create(
        ComparisonPlan plan,
        List<ComparisonPlanDiagnostic> diagnostics,
        List<WindowRecord> selected,
        List<ExcludedWindowRecord> excluded,
        List<NormalizedWindowRecord> normalized)
    {
        return new PreparedComparison(
            plan,
            diagnostics.ToArray(),
            selected.ToArray(),
            excluded.ToArray(),
            normalized.ToArray());
    }

    private static string StableObjectValue(object? value)
    {
        return CanonicalValueFormatter.Format(value);
    }
}
