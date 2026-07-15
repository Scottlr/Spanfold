using Spanfold.Internal.Keys;

namespace Spanfold.Comparison;

/// <summary>
/// Starts comparison workflows over recorded window history.
/// </summary>
public static class WindowHistoryComparisonExtensions
{
    /// <summary>
    /// Starts a staged comparison over recorded window history.
    /// </summary>
    public static WindowComparisonBuilder Compare(this WindowHistory history, string name)
    {
        ArgumentNullException.ThrowIfNull(history);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        return new WindowComparisonBuilder(history, name);
    }

    /// <summary>
    /// Builds a directional source matrix for one recorded window name.
    /// </summary>
    public static SourceMatrixResult CompareSources(
        this WindowHistory history,
        string name,
        string windowName,
        IEnumerable<object> sources)
    {
        ArgumentNullException.ThrowIfNull(history);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);
        ArgumentNullException.ThrowIfNull(sources);

        var orderedSources = sources as object[] ?? sources.ToArray();
        var cells = new List<SourceMatrixCell>(orderedSources.Length * orderedSources.Length);
        var sourceHasWindows = new Dictionary<object, bool>();
        var uniqueSources = new HashSet<object>();
        var matrixWindows = history.Windows
            .Where(window => string.Equals(window.WindowName, windowName, StringComparison.Ordinal))
            .ToArray();

        for (var index = 0; index < orderedSources.Length; index++)
        {
            var source = orderedSources[index];
            ArgumentNullException.ThrowIfNull(source);
            if (!uniqueSources.Add(source))
            {
                throw new ArgumentException("Source matrix identities must be unique.", nameof(sources));
            }

            sourceHasWindows[source] = matrixWindows.Any(window =>
                EqualityComparer<object?>.Default.Equals(window.Source, source));
        }

        for (var targetIndex = 0; targetIndex < orderedSources.Length; targetIndex++)
        {
            var targetSource = orderedSources[targetIndex];
            for (var againstIndex = 0; againstIndex < orderedSources.Length; againstIndex++)
            {
                var againstSource = orderedSources[againstIndex];
                var targetHasWindows = sourceHasWindows[targetSource];
                var againstHasWindows = sourceHasWindows[againstSource];

                if (targetIndex == againstIndex)
                {
                    cells.Add(new SourceMatrixCell(
                        targetSource,
                        againstSource,
                        IsDiagonal: true,
                        targetHasWindows,
                        againstHasWindows,
                        OverlapRowCount: 0,
                        ResidualRowCount: 0,
                        MissingRowCount: 0,
                        CoverageRowCount: 0,
                        CoverageRatio: targetHasWindows ? 1d : null));
                    continue;
                }

                var result = history.Compare(name + " " + targetSource + " vs " + againstSource)
                    .Target(targetSource.ToString() ?? "target", selector => selector.Source(targetSource))
                    .Against(againstSource.ToString() ?? "against", selector => selector.Source(againstSource))
                    .Within(scope => scope.Window(windowName))
                    .Using(comparators => comparators.Overlap().Residual().Missing().Coverage())
                    .Run();

                cells.Add(new SourceMatrixCell(
                    targetSource,
                    againstSource,
                    IsDiagonal: false,
                    targetHasWindows,
                    againstHasWindows,
                    result.OverlapRows.Count,
                    result.ResidualRows.Count,
                    result.MissingRows.Count,
                    result.CoverageRows.Count,
                    GetCoverageRatio(result.CoverageSummaries)));
            }
        }

        return new SourceMatrixResult(name, windowName, orderedSources, cells.ToArray());
    }

    /// <summary>
    /// Compares parent windows against child contribution windows using temporal co-activity.
    /// </summary>
    public static HierarchyComparisonResult CompareHierarchy(
        this WindowHistory history,
        string name,
        string parentWindowName,
        string childWindowName)
    {
        ArgumentNullException.ThrowIfNull(history);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentException.ThrowIfNullOrWhiteSpace(parentWindowName);
        ArgumentException.ThrowIfNullOrWhiteSpace(childWindowName);

        var parents = WindowsForName(history, parentWindowName);
        var children = WindowsForName(history, childWindowName);
        var diagnostics = new List<ComparisonPlanDiagnostic>();

        if (parents.Count == 0)
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.MissingLineage,
                "Hierarchy comparison found no parent windows.",
                "parentWindowName",
                ComparisonPlanDiagnosticSeverity.Warning));
        }

        if (children.Count == 0)
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.MissingLineage,
                "Hierarchy comparison found no child contribution windows.",
                "childWindowName",
                ComparisonPlanDiagnosticSeverity.Warning));
        }

        if (ContainsOpenWindow(parents) || ContainsOpenWindow(children))
        {
            diagnostics.Add(new ComparisonPlanDiagnostic(
                ComparisonPlanValidationCode.HierarchyOpenWindowsWithoutHorizon,
                "Hierarchy co-activity excludes open-window duration because no evaluation horizon was supplied.",
                "windowNames",
                ComparisonPlanDiagnosticSeverity.Warning));
        }

        var rows = new List<HierarchyComparisonRow>();
        foreach (var scope in BuildHierarchyScopes(parents, children))
        {
            AddHierarchyRows(scope.Source, scope.Partition, parents, children, rows);
        }

        return new HierarchyComparisonResult(
            name,
            parentWindowName,
            childWindowName,
            rows.ToArray(),
            diagnostics.ToArray());
    }

    private static List<WindowRecord> WindowsForName(WindowHistory history, string windowName)
    {
        return history.Windows
            .Where(window => string.Equals(window.WindowName, windowName, StringComparison.Ordinal))
            .ToList();
    }

    private static List<HierarchyScope> BuildHierarchyScopes(
        List<WindowRecord> parents,
        List<WindowRecord> children)
    {
        var scopes = new List<HierarchyScope>();
        AddScopes(parents, scopes);
        AddScopes(children, scopes);
        scopes.Sort(static (left, right) =>
        {
            var source = StableObjectValue(left.Source).CompareTo(StableObjectValue(right.Source));
            return source != 0
                ? source
                : StableObjectValue(left.Partition).CompareTo(StableObjectValue(right.Partition));
        });
        return scopes;
    }

    private static void AddScopes(List<WindowRecord> windows, List<HierarchyScope> scopes)
    {
        for (var index = 0; index < windows.Count; index++)
        {
            var scope = new HierarchyScope(windows[index].Source, windows[index].Partition);
            if (!scopes.Contains(scope))
            {
                scopes.Add(scope);
            }
        }
    }

    private static void AddHierarchyRows(
        object? source,
        object? partition,
        List<WindowRecord> parents,
        List<WindowRecord> children,
        List<HierarchyComparisonRow> rows)
    {
        var scopedParents = FilterHierarchyScope(parents, source, partition);
        var scopedChildren = FilterHierarchyScope(children, source, partition);
        var boundaries = new List<TemporalPoint>((scopedParents.Count + scopedChildren.Count) * 2);
        AddBoundaries(scopedParents, boundaries);
        AddBoundaries(scopedChildren, boundaries);
        boundaries.Sort(static (left, right) => left.CompareTo(right));

        var unique = new List<TemporalPoint>(boundaries.Count);
        for (var index = 0; index < boundaries.Count; index++)
        {
            if (unique.Count == 0 || boundaries[index].CompareTo(unique[^1]) != 0)
            {
                unique.Add(boundaries[index]);
            }
        }

        for (var index = 0; index < unique.Count - 1; index++)
        {
            var start = unique[index];
            var end = unique[index + 1];
            if (start.CompareTo(end) >= 0)
            {
                continue;
            }

            var parentIds = ActiveIds(scopedParents, start, end);
            var childIds = ActiveIds(scopedChildren, start, end);
            if (parentIds.Count == 0 && childIds.Count == 0)
            {
                continue;
            }

            rows.Add(new HierarchyComparisonRow(
                GetHierarchyKind(parentIds.Count, childIds.Count),
                source,
                partition,
                TemporalRange.Closed(start, end),
                parentIds,
                childIds));
        }
    }

    private static List<WindowRecord> FilterHierarchyScope(
        List<WindowRecord> windows,
        object? source,
        object? partition)
    {
        return windows
            .Where(window => EqualityComparer<object?>.Default.Equals(window.Source, source)
                && EqualityComparer<object?>.Default.Equals(window.Partition, partition))
            .ToList();
    }

    private static void AddBoundaries(List<WindowRecord> windows, List<TemporalPoint> boundaries)
    {
        for (var index = 0; index < windows.Count; index++)
        {
            var window = windows[index];
            if (window.EndPosition.HasValue)
            {
                boundaries.Add(TemporalPoint.ForPosition(window.StartPosition));
                boundaries.Add(TemporalPoint.ForPosition(window.EndPosition.Value));
            }
        }
    }

    private static bool ContainsOpenWindow(List<WindowRecord> windows)
    {
        return windows.Any(static window => !window.EndPosition.HasValue);
    }

    private static IReadOnlyList<WindowRecordId> ActiveIds(
        List<WindowRecord> windows,
        TemporalPoint start,
        TemporalPoint end)
    {
        var ids = new List<WindowRecordId>();
        for (var index = 0; index < windows.Count; index++)
        {
            var window = windows[index];
            if (!window.EndPosition.HasValue)
            {
                continue;
            }

            var range = TemporalRange.Closed(
                TemporalPoint.ForPosition(window.StartPosition),
                TemporalPoint.ForPosition(window.EndPosition.Value));
            if (range.Start.CompareTo(start) <= 0 && end.CompareTo(range.End!.Value) <= 0)
            {
                ids.Add(window.Id);
            }
        }

        ids.Sort(static (left, right) => string.CompareOrdinal(left.Value, right.Value));
        return ids.ToArray();
    }

    private static HierarchyComparisonRowKind GetHierarchyKind(int parentCount, int childCount)
    {
        if (parentCount > 0 && childCount > 0)
        {
            return HierarchyComparisonRowKind.ParentExplained;
        }

        return parentCount > 0
            ? HierarchyComparisonRowKind.UnexplainedParent
            : HierarchyComparisonRowKind.OrphanChild;
    }

    private static double? GetCoverageRatio(IReadOnlyList<CoverageSummary> summaries)
    {
        var target = 0d;
        var covered = 0d;
        for (var index = 0; index < summaries.Count; index++)
        {
            target += summaries[index].TargetMagnitude;
            covered += summaries[index].CoveredMagnitude;
        }

        return target == 0d ? null : covered / target;
    }

    private static string StableObjectValue(object? value)
    {
        return CanonicalValueFormatter.Format(value);
    }

    private sealed record HierarchyScope(object? Source, object? Partition);
}
