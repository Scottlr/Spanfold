using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class CohortComparisonTests
{
    [Fact]
    public void ResidualAgainstAnyCohortDoesNotDoubleCountAlternatingMembers()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 6);
        AddClosedWindow(pipeline, source: "source-c", start: 6, end: 11);

        var result = pipeline.History
            .Compare("Source A vs cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.Any()))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Empty(result.ResidualRows);
    }

    [Fact]
    public void SegmentFiltersApplyToCohortResidual()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11, tradingState: "Suspended");
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 6, tradingState: "Suspended");
        AddClosedWindow(pipeline, source: "source-c", start: 6, end: 11, tradingState: "Open");

        var result = pipeline.History
            .Compare("Source A suspended vs cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort.Sources("source-b", "source-c"))
            .Within(scope => scope
                .Window("SelectionPriced")
                .Segment("tradingState", "Suspended"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Equal(5, result.ResidualRows.TotalPositionLength());
    }

    [Fact]
    public void ResidualAgainstAllCohortRequiresEveryMemberActive()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 6);
        AddClosedWindow(pipeline, source: "source-c", start: 6, end: 11);

        var result = pipeline.History
            .Compare("Source A vs full cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.All()))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Equal(10, result.ResidualRows.TotalPositionLength());
    }

    [Fact]
    public void ResidualAgainstAtLeastCohortUsesThreshold()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-c", start: 1, end: 6);
        AddClosedWindow(pipeline, source: "source-d", start: 6, end: 11);

        var result = pipeline.History
            .Compare("Source A vs threshold cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c", "source-d")
                .Activity(CohortActivity.AtLeast(2)))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Empty(result.ResidualRows);
    }

    [Fact]
    public void ResidualAgainstAtMostCohortUsesThreshold()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-c", start: 1, end: 6);

        var result = pipeline.History
            .Compare("Source A vs at-most cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.AtMost(1)))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Equal(5, result.ResidualRows.TotalPositionLength());
    }

    [Fact]
    public void ResidualAgainstExactCohortUsesThreshold()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-c", start: 1, end: 6);

        var result = pipeline.History
            .Compare("Source A vs exact cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.Exactly(1)))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Equal(5, result.ResidualRows.TotalPositionLength());
    }

    [Fact]
    public void ResidualAgainstNoneCohortRequiresNoActiveMembers()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 6);

        var result = pipeline.History
            .Compare("Source A vs none cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.None()))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Equal(5, result.ResidualRows.TotalPositionLength());
    }

    [Fact]
    public void NoneCohortRuleExportsWithoutCount()
    {
        var pipeline = CreatePipeline();

        var result = pipeline.History
            .Compare("Source A vs none cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b")
                .Activity(CohortActivity.None()))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Contains("\"activity\": \"none\"", result.ExportJson());
        Assert.Contains("cohort=none", result.ExportMarkdown());
    }

    [Fact]
    public void AtLeastCohortCountCannotExceedDeclaredSources()
    {
        var pipeline = CreatePipeline();

        var exception = Assert.Throws<InvalidOperationException>(() =>
            pipeline.History
                .Compare("Invalid cohort")
                .Target("source-a", selector => selector.Source("source-a"))
                .AgainstCohort("cohort", cohort => cohort
                    .Sources("source-b")
                    .Activity(CohortActivity.AtLeast(2)))
                .Within(scope => scope.Window("SelectionPriced"))
                .Using(comparators => comparators.Residual())
                .Build());

        Assert.Contains("cannot exceed", exception.Message);
    }

    [Fact]
    public void CohortMustDeclareAtLeastOneSource()
    {
        var pipeline = CreatePipeline();

        var exception = Assert.Throws<InvalidOperationException>(() =>
            pipeline.History
                .Compare("Invalid cohort")
                .Target("source-a", selector => selector.Source("source-a"))
                .AgainstCohort("cohort", cohort => cohort)
                .Within(scope => scope.Window("SelectionPriced"))
                .Using(comparators => comparators.Residual())
                .Build());

        Assert.Contains("at least one source", exception.Message);
    }

    [Fact]
    public void DuplicateCohortSourcesAreRejected()
    {
        var pipeline = CreatePipeline();

        var exception = Assert.Throws<InvalidOperationException>(() =>
            pipeline.History
                .Compare("Invalid cohort")
                .Target("source-a", selector => selector.Source("source-a"))
                .AgainstCohort("cohort", cohort => cohort.Sources("source-b", "source-b"))
                .Within(scope => scope.Window("SelectionPriced"))
                .Using(comparators => comparators.Residual())
                .Build());

        Assert.Contains("must be unique", exception.Message);
    }

    [Fact]
    public void DirectCohortSelectorEnforcesPopulationInvariants()
    {
        Assert.Throws<ArgumentException>(() =>
            ComparisonSelector.ForCohortSources(
                ["source-b", "source-b"],
                CohortActivity.All()));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            ComparisonSelector.ForCohortSources(
                ["source-b"],
                CohortActivity.AtLeast(2)));
    }

    [Fact]
    public void NegativeCohortActivityCountsAreRejected()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => CohortActivity.AtMost(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => CohortActivity.Exactly(-1));
    }

    [Fact]
    public void CohortExportAndExplainIncludeActivityRule()
    {
        var pipeline = CreatePipeline();

        var result = pipeline.History
            .Compare("Source A vs threshold cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.AtLeast(2)))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Contains("\"activity\": \"at-least\"", result.ExportJson());
        Assert.Contains("cohort=at-least:2", result.ExportMarkdown());
    }

    [Fact]
    public void CohortResultIncludesSegmentEvidenceMetadata()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 6);

        var result = pipeline.History
            .Compare("Source A vs cohort evidence")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.AtLeast(2)))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Contains(result.ExtensionMetadata, metadata =>
            metadata.ExtensionId == "spanfold.cohort"
            && metadata.Value.Contains("\"required\":2", StringComparison.Ordinal)
            && metadata.Value.Contains("\"isActive\":false", StringComparison.Ordinal));
    }

    [Fact]
    public void CohortEvidenceMetadataCanBeQueriedAsTypedValues()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b;=,", start: 1, end: 6);

        var result = pipeline.History
            .Compare("Source A vs typed cohort evidence")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b;=,", "source-c")
                .Activity(CohortActivity.AtLeast(2)))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        var inactive = Assert.Single(result.CohortEvidence(), evidence =>
            !evidence.IsActive
            && evidence.ActiveCount == 1);

        Assert.Equal("at-least", inactive.Rule);
        Assert.Equal(2, inactive.RequiredCount);
        Assert.Contains("source-b;=,", inactive.ActiveSources);
        Assert.Contains("\"required\":2", inactive.RawValue);
    }

    [Fact]
    public void DebugHtmlShowsCohortEvidenceMetadata()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 6);

        var result = pipeline.History
            .Compare("Source A vs cohort evidence")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.AtLeast(2)))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual())
            .Run();

        var html = result.ExportDebugHtml();

        Assert.Contains("spanfold.cohort", html);
        Assert.Contains("required", html);
        Assert.Contains("isActive", html);
    }

    private static EventPipeline<PriceUpdate> CreatePipeline()
    {
        return Spanfold
            .For<PriceUpdate>()
            .RecordWindows()
            .TrackWindow("SelectionPriced", update => update.SelectionId, update => update.HasPrice);
    }

    private static void AddClosedWindow(
        EventPipeline<PriceUpdate> pipeline,
        string source,
        long start,
        long end,
        string tradingState = "Suspended")
    {
        var open = new WindowEmission<PriceUpdate>(
            "SelectionPriced",
            "selection-1",
            new PriceUpdate("selection-1", HasPrice: true),
            WindowTransitionKind.Opened,
            source,
            Segments:
            [
                new WindowSegment("phase", "InPlay"),
                new WindowSegment("period", "FinalQuarter", ParentName: "phase"),
                new WindowSegment("tradingState", tradingState)
            ]);
        var close = open with
        {
            Event = new PriceUpdate("selection-1", HasPrice: false),
            Kind = WindowTransitionKind.Closed
        };

        pipeline.History.Record([open], start, eventTime: null);
        pipeline.History.Record([close], end, eventTime: null);
    }

    private sealed record PriceUpdate(string SelectionId, bool HasPrice);
}
