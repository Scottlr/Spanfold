using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class SegmentAwareComparisonTests
{
    [Fact]
    public void HistoryCanQueryWindowsBySegment()
    {
        var pipeline = CreateSegmentedPipeline();

        AddClosedWindow(pipeline, source: "source-a", phase: "InPlay", start: 1, end: 3);
        AddClosedWindow(pipeline, source: "source-b", phase: "Pregame", start: 3, end: 5);

        var inPlay = pipeline.History.WithSegment("phase", "InPlay");

        var window = Assert.Single(inPlay);
        Assert.Equal("source-a", window.Source);
    }

    [Fact]
    public void HistoryCanQueryWindowsByTag()
    {
        var pipeline = CreateSegmentedPipeline();

        AddClosedWindow(pipeline, source: "source-a", phase: "InPlay", start: 1, end: 3, fleet: "critical");
        AddClosedWindow(pipeline, source: "source-b", phase: "InPlay", start: 3, end: 5, fleet: "standard");

        var critical = pipeline.History.WithTag("fleet", "critical");

        var window = Assert.Single(critical);
        Assert.Equal("source-a", window.Source);
    }

    [Fact]
    public void ComparisonScopeCanFilterBySegment()
    {
        var pipeline = CreateSegmentedPipeline();

        AddClosedWindow(pipeline, source: "source-a", phase: "InPlay", start: 1, end: 4);
        AddClosedWindow(pipeline, source: "source-b", phase: "Pregame", start: 1, end: 4);

        var result = pipeline.History
            .Compare("In-play residual")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("SelectionPriced").Segment("phase", "InPlay"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Single(result.Prepared!.NormalizedWindows);
        Assert.Single(result.ResidualRows);
        Assert.Empty(result.OverlapRows);
    }

    [Fact]
    public void ComparisonScopeCanFilterByTag()
    {
        var pipeline = CreateSegmentedPipeline();

        AddClosedWindow(pipeline, source: "source-a", phase: "InPlay", start: 1, end: 4, fleet: "critical");
        AddClosedWindow(pipeline, source: "source-b", phase: "InPlay", start: 1, end: 4, fleet: "standard");

        var result = pipeline.History
            .Compare("Critical fleet residual")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("SelectionPriced").Tag("fleet", "critical"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Single(result.Prepared!.NormalizedWindows);
        Assert.Single(result.ResidualRows);
        Assert.Contains("\"tagFilters\"", result.ExportJson());
        Assert.Contains("tags=fleet=System.String:critical", result.Explain());
    }

    [Fact]
    public void ComparisonScopeCanFilterBySegmentAndTag()
    {
        var pipeline = CreateSegmentedPipeline();

        AddClosedWindow(pipeline, source: "source-a", phase: "InPlay", start: 1, end: 4, fleet: "critical");
        AddClosedWindow(pipeline, source: "source-b", phase: "Pregame", start: 1, end: 4, fleet: "critical");

        var result = pipeline.History
            .Compare("Critical in-play residual")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope
                .Window("SelectionPriced")
                .Segment("phase", "InPlay")
                .Tag("fleet", "critical"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Single(result.Prepared!.NormalizedWindows);
        Assert.Single(result.ResidualRows);
    }

    [Fact]
    public void AlignmentDoesNotCombineDifferentSegmentContexts()
    {
        var pipeline = CreateSegmentedPipeline();

        AddClosedWindow(pipeline, source: "source-a", phase: "InPlay", start: 1, end: 4);
        AddClosedWindow(pipeline, source: "source-b", phase: "Pregame", start: 1, end: 4);

        var result = pipeline.History
            .Compare("Segment-aware overlap")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Overlap())
            .Run();

        Assert.Empty(result.OverlapRows);
        Assert.All(result.Aligned!.Segments, segment => Assert.Single(segment.Segments));
    }

    [Fact]
    public void DebugHtmlShowsSegmentContext()
    {
        var pipeline = CreateSegmentedPipeline();

        AddClosedWindow(pipeline, source: "source-a", phase: "InPlay", start: 1, end: 4);
        AddClosedWindow(pipeline, source: "source-b", phase: "InPlay", start: 2, end: 3);

        var result = pipeline.History
            .Compare("Segment debug")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Overlap())
            .Run();

        var html = result.ExportDebugHtml();

        Assert.Contains("phase=InPlay", html);
    }

    [Fact]
    public void DebugHtmlShowsTagsAndBoundaryReasons()
    {
        var pipeline = CreateSegmentedPipeline();

        AddClosedWindow(
            pipeline,
            source: "source-a",
            phase: "Pregame",
            start: 1,
            end: 4,
            fleet: "critical",
            boundaryReason: WindowBoundaryReason.SegmentChanged,
            boundaryChanges: [new WindowBoundaryChange("phase", "Pregame", "InPlay")]);
        AddClosedWindow(pipeline, source: "source-b", phase: "Pregame", start: 1, end: 3);

        var result = pipeline.History
            .Compare("Segment debug details")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Overlap())
            .Run();

        var html = result.ExportDebugHtml();

        Assert.Contains("fleet=critical", html);
        Assert.Contains("SegmentChanged", html);
        Assert.Contains("boundary-marker", html);
        Assert.Contains("phase Pregame -&gt; InPlay", html);
    }

    [Fact]
    public void DebugHtmlShowsNestedSegmentBands()
    {
        var pipeline = CreateSegmentedPipeline();

        AddClosedWindow(pipeline, source: "source-a", phase: "InPlay", start: 1, end: 4, period: "FinalQuarter");
        AddClosedWindow(pipeline, source: "source-b", phase: "InPlay", start: 2, end: 5, period: "FinalQuarter");

        var result = pipeline.History
            .Compare("Nested segment debug")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Overlap())
            .Run();

        var html = result.ExportDebugHtml();

        Assert.Contains("Segment Context Bands", html);
        Assert.Contains("FinalQuarter", html);
        Assert.Contains("parent phase", html);
    }

    private static EventPipeline<PriceUpdate> CreateSegmentedPipeline()
    {
        return EventPipeline
            .For<PriceUpdate>()
            .RecordWindows()
            .TrackWindow("SelectionPriced", update => update.SelectionId, update => update.HasPrice);
    }

    private static void AddClosedWindow(
        EventPipeline<PriceUpdate> pipeline,
        string source,
        string phase,
        long start,
        long end,
        string? fleet = null,
        WindowBoundaryReason? boundaryReason = null,
        IReadOnlyList<WindowBoundaryChange>? boundaryChanges = null,
        string? period = null)
    {
        IReadOnlyList<WindowSegment> segments = period is null
            ? [new WindowSegment("phase", phase)]
            : [new WindowSegment("phase", phase), new WindowSegment("period", period, ParentName: "phase")];
        var open = new WindowEmission<PriceUpdate>(
            "SelectionPriced",
            "selection-1",
            new PriceUpdate("selection-1", HasPrice: true),
            WindowTransitionKind.Opened,
            source,
            Segments: segments,
            Tags: fleet is null ? [] : [new WindowTag("fleet", fleet)]);
        var close = new WindowEmission<PriceUpdate>(
            open.WindowName,
            open.Key,
            new PriceUpdate("selection-1", HasPrice: false),
            WindowTransitionKind.Closed,
            open.Source,
            open.Partition,
            open.Segments,
            open.Tags,
            boundaryReason,
            boundaryChanges);

        pipeline.History.Record([open], start, eventTime: null);
        pipeline.History.Record([close], end, eventTime: null);
    }

    private sealed record PriceUpdate(string SelectionId, bool HasPrice);
}
