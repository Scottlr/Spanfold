namespace Spanfold.Internal.Analysis;

internal static class WindowScopeMatcher
{
    internal static bool Matches(WindowRecord window, ComparisonScope scope)
    {
        if (scope.WindowName is not null
            && !string.Equals(window.WindowName, scope.WindowName, StringComparison.Ordinal))
        {
            return false;
        }

        for (var i = 0; i < scope.SegmentFilters.Count; i++)
        {
            if (!HasSegment(window, scope.SegmentFilters[i]))
            {
                return false;
            }
        }

        for (var i = 0; i < scope.TagFilters.Count; i++)
        {
            if (!HasTag(window, scope.TagFilters[i]))
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
}
