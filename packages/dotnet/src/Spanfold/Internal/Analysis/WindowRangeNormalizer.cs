namespace Spanfold.Internal.Analysis;

internal enum WindowRangeNormalizationFailureKind
{
    FutureWindowExcluded = 0,
    MissingEventTime = 1,
    OpenWindowWithoutPolicy = 2,
    MixedTimeAxes = 3,
    InvalidRangeDuration = 4
}

internal sealed record WindowRangeNormalizationFailure(
    WindowRangeNormalizationFailureKind Kind,
    string Reason);

internal readonly record struct NormalizedWindowRange(
    TemporalRange Range,
    ComparisonFinality Finality);

internal static class WindowRangeNormalizer
{
    internal static bool TryNormalize(
        WindowRecord window,
        ComparisonNormalizationPolicy policy,
        TemporalPoint? knownAt,
        out NormalizedWindowRange normalized,
        out WindowRangeNormalizationFailure? failure)
    {
        normalized = default;
        failure = null;

        if (knownAt.HasValue && window.StartPosition > knownAt.Value.Position)
        {
            failure = new WindowRangeNormalizationFailure(
                WindowRangeNormalizationFailureKind.FutureWindowExcluded,
                "Window was not available at the configured known-at point.");
            return false;
        }

        return policy.TimeAxis == TemporalAxis.Timestamp
            ? TryNormalizeTimestamp(window, policy, out normalized, out failure)
            : TryNormalizePosition(window, policy, knownAt, out normalized, out failure);
    }

    private static bool TryNormalizePosition(
        WindowRecord window,
        ComparisonNormalizationPolicy policy,
        TemporalPoint? knownAt,
        out NormalizedWindowRange normalized,
        out WindowRangeNormalizationFailure? failure)
    {
        var start = TemporalPoint.ForPosition(window.StartPosition);

        if (knownAt.HasValue
            && (!window.EndPosition.HasValue || knownAt.Value.Position < window.EndPosition.Value))
        {
            normalized = Provisional(start, knownAt.Value);
            failure = null;
            return true;
        }

        if (window.EndPosition.HasValue)
        {
            normalized = Final(TemporalRange.Closed(
                start,
                TemporalPoint.ForPosition(window.EndPosition.Value)));
            failure = null;
            return true;
        }

        return TryNormalizeOpenWindow(start, policy, out normalized, out failure);
    }

    private static bool TryNormalizeTimestamp(
        WindowRecord window,
        ComparisonNormalizationPolicy policy,
        out NormalizedWindowRange normalized,
        out WindowRangeNormalizationFailure? failure)
    {
        if (!window.StartTime.HasValue || (!window.EndTime.HasValue && window.IsClosed))
        {
            normalized = default;
            failure = new WindowRangeNormalizationFailure(
                WindowRangeNormalizationFailureKind.MissingEventTime,
                "Event-time comparison requires recorded event timestamps.");
            return false;
        }

        var start = TemporalPoint.ForTimestamp(window.StartTime.Value, window.TimestampClock);
        if (window.EndTime.HasValue)
        {
            normalized = Final(TemporalRange.Closed(
                start,
                TemporalPoint.ForTimestamp(window.EndTime.Value, window.TimestampClock)));
            failure = null;
            return true;
        }

        return TryNormalizeOpenWindow(start, policy, out normalized, out failure);
    }

    private static bool TryNormalizeOpenWindow(
        TemporalPoint start,
        ComparisonNormalizationPolicy policy,
        out NormalizedWindowRange normalized,
        out WindowRangeNormalizationFailure? failure)
    {
        if (policy.OpenWindowPolicy != ComparisonOpenWindowPolicy.ClipToHorizon
            || !policy.OpenWindowHorizon.HasValue)
        {
            normalized = default;
            failure = new WindowRangeNormalizationFailure(
                WindowRangeNormalizationFailureKind.OpenWindowWithoutPolicy,
                "Open windows require an explicit clipping policy.");
            return false;
        }

        var horizon = policy.OpenWindowHorizon.Value;
        if (horizon.Axis != start.Axis)
        {
            normalized = default;
            failure = new WindowRangeNormalizationFailure(
                WindowRangeNormalizationFailureKind.MixedTimeAxes,
                "Open-window horizon must use the same temporal axis as the normalized range.");
            return false;
        }

        if (horizon.CompareTo(start) < 0)
        {
            normalized = default;
            failure = new WindowRangeNormalizationFailure(
                WindowRangeNormalizationFailureKind.InvalidRangeDuration,
                "Open-window horizon cannot be earlier than the window start.");
            return false;
        }

        normalized = Provisional(start, horizon);
        failure = null;
        return true;
    }

    private static NormalizedWindowRange Final(TemporalRange range)
    {
        return new NormalizedWindowRange(range, ComparisonFinality.Final);
    }

    private static NormalizedWindowRange Provisional(TemporalPoint start, TemporalPoint end)
    {
        return new NormalizedWindowRange(
            TemporalRange.WithEffectiveEnd(start, end, TemporalRangeEndStatus.OpenAtHorizon),
            ComparisonFinality.Provisional);
    }
}
