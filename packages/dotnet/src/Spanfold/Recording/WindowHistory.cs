using Spanfold.Internal.Recording;

namespace Spanfold;

/// <summary>
/// Stores recorded open and closed windows and exposes window queries.
/// </summary>
/// <remarks>
/// History is append-oriented from pipeline ingestion. Query and comparison
/// APIs return materialized snapshots so callers can inspect the current
/// recorded state without mutating the active runtime.
/// </remarks>
public sealed class WindowHistory
{
    private readonly bool enabled;
    private readonly IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers;
    private readonly Dictionary<WindowRecordingKey, OpenWindow> openWindows;
    private readonly List<ClosedWindow> closedWindows;
    private readonly List<WindowAnnotation> annotations;

    internal WindowHistory(bool enabled)
        : this(enabled, new Dictionary<string, IEqualityComparer<object>>(StringComparer.Ordinal))
    {
    }

    internal WindowHistory(
        bool enabled,
        IReadOnlyDictionary<string, IEqualityComparer<object>> keyComparers)
    {
        this.enabled = enabled;
        this.keyComparers = keyComparers;
        this.openWindows = [];
        this.closedWindows = [];
        this.annotations = [];
    }

    /// <summary>
    /// Gets closed windows recorded by the pipeline.
    /// </summary>
    public IReadOnlyList<ClosedWindow> ClosedWindows => this.closedWindows.ToArray();

    /// <summary>
    /// Gets all recorded windows, including closed windows and currently open windows.
    /// </summary>
    public IReadOnlyList<WindowRecord> Windows
    {
        get
        {
            var windows = new WindowRecord[this.closedWindows.Count + this.openWindows.Count];
            var index = 0;

            foreach (var window in this.closedWindows)
            {
                windows[index] = window;
                index++;
            }

            foreach (var window in this.openWindows.Values)
            {
                windows[index] = window;
                index++;
            }

            return windows;
        }
    }

    /// <summary>
    /// Gets currently open windows recorded by the pipeline.
    /// </summary>
    public IReadOnlyList<OpenWindow> OpenWindows
    {
        get
        {
            var windows = new OpenWindow[this.openWindows.Count];
            var index = 0;

            foreach (var window in this.openWindows.Values)
            {
                windows[index] = window;
                index++;
            }

            return windows;
        }
    }

    /// <summary>
    /// Gets annotations attached to recorded windows.
    /// </summary>
    public IReadOnlyList<WindowAnnotation> Annotations => this.annotations.ToArray();

    /// <summary>
    /// Starts a staged comparison over this recorded window history.
    /// </summary>
    /// <param name="name">A human-readable name for the comparison.</param>
    /// <returns>A comparison builder for the recorded history.</returns>
    /// <exception cref="ArgumentException">
    /// Thrown when <paramref name="name" /> is empty or only whitespace.
    /// </exception>
    public WindowComparisonBuilder Compare(string name)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        return new WindowComparisonBuilder(this, name);
    }

    /// <summary>
    /// Starts a read-only query over recorded windows.
    /// </summary>
    /// <remarks>
    /// Use this API for direct state-history inspection when no cross-source
    /// comparator rows are needed.
    /// </remarks>
    /// <returns>A window history query builder.</returns>
    public WindowHistoryQuery Query()
    {
        return new WindowHistoryQuery(this);
    }

    /// <summary>
    /// Evaluates recorded windows at an explicit horizon.
    /// </summary>
    /// <remarks>
    /// Windows active at the horizon are clipped to the horizon and marked
    /// provisional in the returned snapshot. The underlying history is not
    /// mutated.
    /// </remarks>
    /// <param name="horizon">The horizon used to evaluate the history.</param>
    /// <returns>A read-only snapshot of the recorded history at the horizon.</returns>
    public WindowHistorySnapshot SnapshotAt(TemporalPoint horizon)
    {
        return WindowHistorySnapshot.Create(this, horizon);
    }

    /// <summary>
    /// Attaches external metadata to a recorded window.
    /// </summary>
    /// <remarks>
    /// Annotation is append-only. It does not mutate, split, or revise the
    /// source window. The annotation target uses the window start identity so
    /// metadata attached while a window is open remains associated after the
    /// window closes.
    /// </remarks>
    /// <param name="window">The window to annotate.</param>
    /// <param name="name">The annotation name.</param>
    /// <param name="value">The annotation value.</param>
    /// <param name="knownAt">When the annotation became known, if supplied.</param>
    /// <returns>The appended annotation.</returns>
    public WindowAnnotation Annotate(
        WindowRecord window,
        string name,
        object? value,
        TemporalPoint? knownAt = null)
    {
        ArgumentNullException.ThrowIfNull(window);

        return Annotate(WindowAnnotationTarget.From(window), name, value, knownAt);
    }

    /// <summary>
    /// Attaches external metadata to a window annotation target.
    /// </summary>
    /// <param name="target">The stable window start identity to annotate.</param>
    /// <param name="name">The annotation name.</param>
    /// <param name="value">The annotation value.</param>
    /// <param name="knownAt">When the annotation became known, if supplied.</param>
    /// <returns>The appended annotation.</returns>
    public WindowAnnotation Annotate(
        WindowAnnotationTarget target,
        string name,
        object? value,
        TemporalPoint? knownAt = null)
    {
        ArgumentNullException.ThrowIfNull(target);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        if (knownAt is { Axis: TemporalAxis.Unknown })
        {
            throw new ArgumentException("Annotation known-at point must use a known temporal axis.", nameof(knownAt));
        }

        var revision = 1;
        for (var i = 0; i < this.annotations.Count; i++)
        {
            var annotation = this.annotations[i];
            if (annotation.Target == target
                && string.Equals(annotation.Name, name, StringComparison.Ordinal))
            {
                revision++;
            }
        }

        var appended = new WindowAnnotation(target, name, value, knownAt, revision);
        this.annotations.Add(appended);
        return appended;
    }

    /// <summary>
    /// Gets annotations attached to a recorded window.
    /// </summary>
    /// <param name="window">The recorded window.</param>
    /// <returns>Matching annotations in append order.</returns>
    public IReadOnlyList<WindowAnnotation> AnnotationsFor(WindowRecord window)
    {
        ArgumentNullException.ThrowIfNull(window);

        return AnnotationsFor(WindowAnnotationTarget.From(window));
    }

    /// <summary>
    /// Gets annotations for a recorded window that were known at or before a horizon.
    /// </summary>
    /// <remarks>
    /// Annotations without a comparable known-at point are excluded from this
    /// point-in-time-safe view.
    /// </remarks>
    /// <param name="window">The recorded window.</param>
    /// <param name="horizon">The known-at horizon.</param>
    /// <returns>Matching annotations in append order.</returns>
    public IReadOnlyList<WindowAnnotation> AnnotationsKnownAt(
        WindowRecord window,
        TemporalPoint horizon)
    {
        ArgumentNullException.ThrowIfNull(window);

        return AnnotationsKnownAt(WindowAnnotationTarget.From(window), horizon);
    }

    /// <summary>
    /// Gets annotations attached to a window annotation target.
    /// </summary>
    /// <param name="target">The stable window start identity.</param>
    /// <returns>Matching annotations in append order.</returns>
    public IReadOnlyList<WindowAnnotation> AnnotationsFor(WindowAnnotationTarget target)
    {
        ArgumentNullException.ThrowIfNull(target);

        var matches = new List<WindowAnnotation>();
        for (var i = 0; i < this.annotations.Count; i++)
        {
            if (this.annotations[i].Target == target)
            {
                matches.Add(this.annotations[i]);
            }
        }

        return matches.ToArray();
    }

    /// <summary>
    /// Gets annotations for a window target that were known at or before a horizon.
    /// </summary>
    /// <remarks>
    /// Annotations without a comparable known-at point are excluded from this
    /// point-in-time-safe view.
    /// </remarks>
    /// <param name="target">The stable window start identity.</param>
    /// <param name="horizon">The known-at horizon.</param>
    /// <returns>Matching annotations in append order.</returns>
    public IReadOnlyList<WindowAnnotation> AnnotationsKnownAt(
        WindowAnnotationTarget target,
        TemporalPoint horizon)
    {
        ArgumentNullException.ThrowIfNull(target);

        if (horizon.Axis == TemporalAxis.Unknown)
        {
            throw new ArgumentException("Annotation known-at horizon must use a known temporal axis.", nameof(horizon));
        }

        var matches = new List<WindowAnnotation>();
        for (var i = 0; i < this.annotations.Count; i++)
        {
            var annotation = this.annotations[i];
            if (annotation.Target == target && IsKnownAtOrBefore(annotation, horizon))
            {
                matches.Add(annotation);
            }
        }

        return matches.ToArray();
    }

    private static bool IsKnownAtOrBefore(
        WindowAnnotation annotation,
        TemporalPoint horizon)
    {
        return annotation.KnownAt is { } knownAt
            && knownAt.Axis == horizon.Axis
            && (knownAt.Axis != TemporalAxis.Timestamp
                || string.Equals(knownAt.Clock, horizon.Clock, StringComparison.Ordinal))
            && knownAt.CompareTo(horizon) <= 0;
    }

    /// <summary>
    /// Gets recorded windows for a configured window name.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <returns>Recorded windows with the supplied window name.</returns>
    public IReadOnlyList<WindowRecord> ForWindow(string windowName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);

        return Windows
            .Where(window => string.Equals(window.WindowName, windowName, StringComparison.Ordinal))
            .ToArray();
    }

    /// <summary>
    /// Gets recorded windows that contain a required segment value.
    /// </summary>
    /// <param name="name">The segment dimension name.</param>
    /// <param name="value">The required segment value.</param>
    /// <returns>Recorded windows that contain the required segment value.</returns>
    public IReadOnlyList<WindowRecord> WithSegment(string name, object? value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        return Windows
            .Where(window => HasSegment(window, name, value))
            .ToArray();
    }

    /// <summary>
    /// Gets recorded windows that contain a required tag value.
    /// </summary>
    /// <param name="name">The tag name.</param>
    /// <param name="value">The required tag value.</param>
    /// <returns>Recorded windows that contain the required tag value.</returns>
    public IReadOnlyList<WindowRecord> WithTag(string name, object? value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);

        return Windows
            .Where(window => HasTag(window, name, value))
            .ToArray();
    }

    /// <summary>
    /// Builds a directional source matrix for one recorded window name.
    /// </summary>
    /// <remarks>
    /// Cells are emitted in row-major source order. Each non-diagonal cell
    /// treats the row source as target and the column source as comparison.
    /// Diagonal cells are identity rows and do not run pairwise comparators.
    /// </remarks>
    /// <param name="name">A human-readable matrix name.</param>
    /// <param name="windowName">The recorded window name to compare.</param>
    /// <param name="sources">The sources to include, in row and column order.</param>
    /// <returns>A directional source matrix.</returns>
    public SourceMatrixResult CompareSources(
        string name,
        string windowName,
        IEnumerable<object> sources)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentException.ThrowIfNullOrWhiteSpace(windowName);
        ArgumentNullException.ThrowIfNull(sources);

        var orderedSources = sources as object[] ?? sources.ToArray();
        var cells = new List<SourceMatrixCell>(orderedSources.Length * orderedSources.Length);
        var sourceHasWindows = new Dictionary<object, bool>();
        var uniqueSources = new HashSet<object>();

        for (var i = 0; i < orderedSources.Length; i++)
        {
            var source = orderedSources[i];
            ArgumentNullException.ThrowIfNull(source);
            if (!uniqueSources.Add(source))
            {
                throw new ArgumentException("Source matrix identities must be unique.", nameof(sources));
            }

            sourceHasWindows[source] = HasWindowForSource(windowName, source);
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

                var result = Compare(name + " " + targetSource + " vs " + againstSource)
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
    /// Compares parent windows against child contribution windows.
    /// </summary>
    /// <remarks>
    /// Lineage is inferred by matching source and partition for the supplied
    /// parent and child window names. Parent and child keys may differ because
    /// rollups often aggregate several child keys into one parent key.
    /// </remarks>
    /// <param name="name">A human-readable comparison name.</param>
    /// <param name="parentWindowName">The parent recorded window name.</param>
    /// <param name="childWindowName">The child contribution window name.</param>
    /// <returns>A hierarchy comparison result.</returns>
    public HierarchyComparisonResult CompareHierarchy(
        string name,
        string parentWindowName,
        string childWindowName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentException.ThrowIfNullOrWhiteSpace(parentWindowName);
        ArgumentException.ThrowIfNullOrWhiteSpace(childWindowName);

        var parents = WindowsForName(parentWindowName);
        var children = WindowsForName(childWindowName);
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

        var rows = new List<HierarchyComparisonRow>();
        var scopes = BuildHierarchyScopes(parents, children);

        foreach (var scope in scopes)
        {
            AddHierarchyRows(
                scope.Source,
                scope.Partition,
                parents,
                children,
                rows);
        }

        return new HierarchyComparisonResult(
            name,
            parentWindowName,
            childWindowName,
            rows.ToArray(),
            diagnostics.ToArray());
    }

    /// <summary>
    /// Finds overlapping closed windows within the same window scope.
    /// </summary>
    /// <returns>The overlapping window pairs.</returns>
    public IReadOnlyList<WindowOverlap> FindOverlaps()
    {
        var overlaps = new List<WindowOverlap>();

        for (var i = 0; i < this.closedWindows.Count; i++)
        {
            var first = this.closedWindows[i];

            for (var j = i + 1; j < this.closedWindows.Count; j++)
            {
                var second = this.closedWindows[j];
                if (!IsSameScope(first, second) || !Overlaps(first, second))
                {
                    continue;
                }

                overlaps.Add(new WindowOverlap(first, second));
            }
        }

        return overlaps.ToArray();
    }

    /// <summary>
    /// Finds target-source window segments that are not covered by comparison sources.
    /// </summary>
    /// <param name="targetSource">The source whose unique residual segments should be returned.</param>
    /// <returns>The residual segments for the target source.</returns>
    public IReadOnlyList<WindowResidualSegment> FindResiduals(object targetSource)
    {
        ArgumentNullException.ThrowIfNull(targetSource);

        var residuals = new List<WindowResidualSegment>();

        foreach (var target in this.closedWindows)
        {
            if (!EqualityComparer<object?>.Default.Equals(target.Source, targetSource))
            {
                continue;
            }

            var segments = new List<PositionSegment>
            {
                new(target.StartPosition, ClosedEndPosition(target))
            };

            foreach (var comparison in this.closedWindows)
            {
                if (ReferenceEquals(target, comparison)
                    || EqualityComparer<object?>.Default.Equals(comparison.Source, targetSource)
                    || !IsSameScope(target, comparison)
                    || !Overlaps(target, comparison))
                {
                    continue;
                }

                Subtract(segments, comparison);
            }

            foreach (var segment in segments)
            {
                if (segment.Start >= segment.End)
                {
                    continue;
                }

                residuals.Add(new WindowResidualSegment(
                    target.WindowName,
                    target.Key,
                    targetSource,
                    segment.Start,
                    segment.End,
                    target.Partition));
            }
        }

        return residuals.ToArray();
    }

    internal void Record<TEvent>(
        IReadOnlyList<WindowEmission<TEvent>> emissions,
        long processingPosition,
        DateTimeOffset? eventTime)
    {
        if (!this.enabled)
        {
            return;
        }

        foreach (var emission in emissions)
        {
            var key = new WindowRecordingKey(
                emission.WindowName,
                emission.Key,
                emission.Source,
                emission.Partition,
                StableSegments(emission.Segments));

            if (emission.Kind == WindowTransitionKind.Opened)
            {
                this.openWindows[key] = new OpenWindow(
                    emission.WindowName,
                    emission.Key,
                    processingPosition,
                    emission.Source,
                    emission.Partition,
                    eventTime,
                    emission.Segments,
                    emission.Tags);
                continue;
            }

            if (!this.TryRemoveOpenWindow(key, out var open))
            {
                continue;
            }

            this.closedWindows.Add(new ClosedWindow(
                open.WindowName,
                open.Key,
                open.StartPosition,
                processingPosition,
                open.Source,
                open.Partition,
                open.StartTime,
                eventTime,
                open.Segments,
                open.Tags,
                emission.BoundaryReason,
                emission.BoundaryChanges));
        }
    }

    private bool TryRemoveOpenWindow(WindowRecordingKey key, out OpenWindow open)
    {
        open = null!;
        if (this.openWindows.Remove(key, out open!))
        {
            return true;
        }

        if (!this.keyComparers.TryGetValue(key.WindowName, out var comparer))
        {
            return false;
        }

        foreach (var candidate in this.openWindows.Keys)
        {
            if (!string.Equals(candidate.WindowName, key.WindowName, StringComparison.Ordinal)
                || !comparer.Equals(candidate.Key, key.Key)
                || !EqualityComparer<object?>.Default.Equals(candidate.Source, key.Source)
                || !EqualityComparer<object?>.Default.Equals(candidate.Partition, key.Partition)
                || !string.Equals(candidate.SegmentContext, key.SegmentContext, StringComparison.Ordinal))
            {
                continue;
            }

            return this.openWindows.Remove(candidate, out open!);
        }

        return false;
    }

    private bool HasWindowForSource(string windowName, object source)
    {
        foreach (var window in Windows)
        {
            if (string.Equals(window.WindowName, windowName, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(window.Source, source))
            {
                return true;
            }
        }

        return false;
    }

    private List<WindowRecord> WindowsForName(string windowName)
    {
        var matches = new List<WindowRecord>();

        foreach (var window in Windows)
        {
            if (string.Equals(window.WindowName, windowName, StringComparison.Ordinal))
            {
                matches.Add(window);
            }
        }

        return matches;
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
            if (source != 0)
            {
                return source;
            }

            return StableObjectValue(left.Partition).CompareTo(StableObjectValue(right.Partition));
        });

        return scopes;
    }

    private static void AddScopes(List<WindowRecord> windows, List<HierarchyScope> scopes)
    {
        for (var i = 0; i < windows.Count; i++)
        {
            var scope = new HierarchyScope(windows[i].Source, windows[i].Partition);
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
        for (var i = 0; i < boundaries.Count; i++)
        {
            if (unique.Count == 0 || boundaries[i].CompareTo(unique[^1]) != 0)
            {
                unique.Add(boundaries[i]);
            }
        }

        for (var i = 0; i < unique.Count - 1; i++)
        {
            var start = unique[i];
            var end = unique[i + 1];
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
        var matches = new List<WindowRecord>();

        for (var i = 0; i < windows.Count; i++)
        {
            var window = windows[i];
            if (EqualityComparer<object?>.Default.Equals(window.Source, source)
                && EqualityComparer<object?>.Default.Equals(window.Partition, partition))
            {
                matches.Add(window);
            }
        }

        return matches;
    }

    private static void AddBoundaries(List<WindowRecord> windows, List<TemporalPoint> boundaries)
    {
        for (var i = 0; i < windows.Count; i++)
        {
            var window = windows[i];
            if (!window.EndPosition.HasValue)
            {
                continue;
            }

            boundaries.Add(TemporalPoint.ForPosition(window.StartPosition));
            boundaries.Add(TemporalPoint.ForPosition(window.EndPosition.Value));
        }
    }

    private static IReadOnlyList<WindowRecordId> ActiveIds(
        List<WindowRecord> windows,
        TemporalPoint start,
        TemporalPoint end)
    {
        var ids = new List<WindowRecordId>();

        for (var i = 0; i < windows.Count; i++)
        {
            var window = windows[i];
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

        for (var i = 0; i < summaries.Count; i++)
        {
            target += summaries[i].TargetMagnitude;
            covered += summaries[i].CoveredMagnitude;
        }

        return target == 0d ? null : covered / target;
    }

    private static string StableObjectValue(object? value)
    {
        return value switch
        {
            null => "<null>",
            IFormattable formattable => value.GetType().FullName + ":" + formattable.ToString(null, System.Globalization.CultureInfo.InvariantCulture),
            _ => value.GetType().FullName + ":" + value
        };
    }

    private static string StableSegments(IReadOnlyList<WindowSegment> segments)
    {
        if (segments.Count == 0)
        {
            return string.Empty;
        }

        var builder = new System.Text.StringBuilder();
        for (var i = 0; i < segments.Count; i++)
        {
            var segment = segments[i];
            builder
                .Append(segment.ParentName ?? string.Empty)
                .Append('/')
                .Append(segment.Name)
                .Append('=')
                .Append(StableObjectValue(segment.Value))
                .Append(';');
        }

        return builder.ToString();
    }

    private static bool IsSameScope(ClosedWindow first, ClosedWindow second)
    {
        return string.Equals(first.WindowName, second.WindowName, StringComparison.Ordinal)
            && EqualityComparer<object>.Default.Equals(first.Key, second.Key)
            && EqualityComparer<object?>.Default.Equals(first.Partition, second.Partition)
            && string.Equals(StableSegments(first.Segments), StableSegments(second.Segments), StringComparison.Ordinal);
    }

    private static bool HasSegment(WindowRecord window, string name, object? value)
    {
        for (var i = 0; i < window.Segments.Count; i++)
        {
            var segment = window.Segments[i];
            if (string.Equals(segment.Name, name, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(segment.Value, value))
            {
                return true;
            }
        }

        return false;
    }

    private static bool HasTag(WindowRecord window, string name, object? value)
    {
        for (var i = 0; i < window.Tags.Count; i++)
        {
            var tag = window.Tags[i];
            if (string.Equals(tag.Name, name, StringComparison.Ordinal)
                && EqualityComparer<object?>.Default.Equals(tag.Value, value))
            {
                return true;
            }
        }

        return false;
    }

    private static bool Overlaps(ClosedWindow first, ClosedWindow second)
    {
        return first.StartPosition < ClosedEndPosition(second)
            && second.StartPosition < ClosedEndPosition(first);
    }

    private static void Subtract(List<PositionSegment> segments, ClosedWindow comparison)
    {
        for (var i = segments.Count - 1; i >= 0; i--)
        {
            var segment = segments[i];
            var overlapStart = Math.Max(segment.Start, comparison.StartPosition);
            var overlapEnd = Math.Min(segment.End, ClosedEndPosition(comparison));

            if (overlapStart >= overlapEnd)
            {
                continue;
            }

            segments.RemoveAt(i);

            if (segment.Start < overlapStart)
            {
                segments.Insert(i, new PositionSegment(segment.Start, overlapStart));
                i++;
            }

            if (overlapEnd < segment.End)
            {
                segments.Insert(i, new PositionSegment(overlapEnd, segment.End));
            }
        }
    }

    private static long ClosedEndPosition(ClosedWindow window)
    {
        return window.EndPosition
            ?? throw new InvalidOperationException("Closed windows must have an end position.");
    }

    private readonly record struct PositionSegment(long Start, long End);

    private sealed record HierarchyScope(object? Source, object? Partition);
}
