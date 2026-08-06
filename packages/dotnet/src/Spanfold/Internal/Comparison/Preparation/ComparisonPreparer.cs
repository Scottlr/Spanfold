using System.Globalization;

using Spanfold;
using Spanfold.Internal.Analysis;
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
            return Create(history, plan, diagnostics, selected, excluded, normalized);
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
            var keyComparer = history.GetKeyComparer(window.WindowName);
            if (canFilterByKnownAt
                && !WindowRangeNormalizer.TryNormalize(
                    window,
                    plan.Normalization,
                    knownAtFilter,
                    out _,
                    out var knownAtFailure)
                && knownAtFailure?.Kind == WindowRangeNormalizationFailureKind.FutureWindowExcluded)
            {
                AddNormalizationFailure(window, plan.Normalization, knownAtFailure, diagnostics, excluded);
                continue;
            }

            if (!WindowScopeMatcher.Matches(window, plan.Scope))
            {
                excluded.Add(new ExcludedWindowRecord(window, "Window is outside the comparison scope."));
                continue;
            }

            var matched = false;
            if (plan.Target.Value.Matches(window, keyComparer))
            {
                matched = true;
                AddNormalized(window, plan.Target.Value.Name, ComparisonSide.Target, plan, knownAtFilter, canFilterByKnownAt, diagnostics, selected, excluded, normalized, memberships);
            }

            for (var i = 0; i < plan.Against.Count; i++)
            {
                var selector = plan.Against[i];
                if (!selector.Matches(window, keyComparer))
                {
                    continue;
                }

                matched = true;
                AddNormalized(window, selector.Name, ComparisonSide.Against, plan, knownAtFilter, canFilterByKnownAt, diagnostics, selected, excluded, normalized, memberships);
            }

            if (!matched)
            {
                excluded.Add(new ExcludedWindowRecord(window, "Window did not match target or comparison selectors."));
            }
        }

        return Create(history, plan, diagnostics, selected, excluded, normalized);
    }

    private static void AddNormalized(
        WindowRecord window,
        string selectorName,
        ComparisonSide side,
        ComparisonPlan plan,
        TemporalPoint knownAt,
        bool canFilterByKnownAt,
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

        if (!WindowRangeNormalizer.TryNormalize(
            window,
            plan.Normalization,
            canFilterByKnownAt ? knownAt : null,
            out var normalizedRange,
            out var failure))
        {
            AddNormalizationFailure(window, plan.Normalization, failure!, diagnostics, excluded);
            return;
        }

        if (!selected.Contains(window))
        {
            selected.Add(window);
        }

        normalized.Add(new NormalizedWindowRecord(
            window,
            window.Id,
            selectorName,
            side,
            normalizedRange.Range,
            window.Segments));
    }

    private static void AddNormalizationFailure(
        WindowRecord window,
        ComparisonNormalizationPolicy policy,
        WindowRangeNormalizationFailure failure,
        List<ComparisonPlanDiagnostic> diagnostics,
        List<ExcludedWindowRecord> excluded)
    {
        var (code, severity) = failure.Kind switch
        {
            WindowRangeNormalizationFailureKind.FutureWindowExcluded => (
                ComparisonPlanValidationCode.FutureWindowExcluded,
                ComparisonPlanDiagnosticSeverity.Warning),
            WindowRangeNormalizationFailureKind.MissingEventTime => (
                ComparisonPlanValidationCode.MissingEventTime,
                policy.NullTimestampPolicy == ComparisonNullTimestampPolicy.Reject
                    ? ComparisonPlanDiagnosticSeverity.Error
                    : ComparisonPlanDiagnosticSeverity.Warning),
            WindowRangeNormalizationFailureKind.OpenWindowWithoutPolicy => (
                ComparisonPlanValidationCode.OpenWindowsWithoutPolicy,
                ComparisonPlanDiagnosticSeverity.Error),
            WindowRangeNormalizationFailureKind.MixedTimeAxes => (
                ComparisonPlanValidationCode.MixedTimeAxes,
                ComparisonPlanDiagnosticSeverity.Error),
            WindowRangeNormalizationFailureKind.InvalidRangeDuration => (
                ComparisonPlanValidationCode.InvalidRangeDuration,
                ComparisonPlanDiagnosticSeverity.Error),
            _ => throw new ArgumentOutOfRangeException(nameof(failure), failure.Kind, "Unknown normalization failure kind.")
        };

        AddExclusion(window, failure.Reason, code, diagnostics, excluded, severity);
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

    private static PreparedComparison Create(
        WindowHistory history,
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
            normalized.ToArray(),
            history.KeyComparers);
    }

    private static string StableObjectValue(object? value)
    {
        return CanonicalValueFormatter.Format(value);
    }
}
