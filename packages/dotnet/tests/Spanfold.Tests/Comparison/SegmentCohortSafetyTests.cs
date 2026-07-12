using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class SegmentCohortSafetyTests
{
    [Fact]
    public void ClosedPriorSegmentIsFinalWhileCurrentLiveSegmentIsProvisional()
    {
        var pipeline = Spanfold
            .For<PriceUpdate>()
            .RecordWindows()
            .TrackWindow("SelectionPriced", window => window
                .Key(update => update.SelectionId)
                .ActiveWhen(update => update.HasPrice)
                .Segment("phase", phase => phase.Value(update => update.Phase)));

        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "Pregame"), source: "source-a");
        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay"), source: "source-a");

        var pregame = pipeline.History
            .Compare("Pregame finality")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("SelectionPriced").Segment("phase", "Pregame"))
            .Using(comparators => comparators.Residual())
            .Run();
        var live = pipeline.History
            .Compare("In-play finality")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("SelectionPriced").Segment("phase", "InPlay"))
            .Using(comparators => comparators.Residual())
            .RunLive(TemporalPoint.ForPosition(3));

        Assert.All(pregame.RowFinalities, finality => Assert.Equal(ComparisonFinality.Final, finality.Finality));
        Assert.Contains(live.RowFinalities, finality => finality.Finality == ComparisonFinality.Provisional);
    }

    [Fact]
    public void KnownAtFilteringRunsBeforeCohortMaterialization()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 6);
        AddClosedWindow(pipeline, source: "source-c", start: 6, end: 11);

        var knownAtFive = pipeline.History
            .Compare("Known-at cohort")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort.Sources("source-b", "source-c"))
            .Within(scope => scope.Window("SelectionPriced"))
            .Normalize(normalization => normalization.KnownAtPosition(5))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Equal(2, knownAtFive.Prepared!.NormalizedWindows.Count);
        Assert.All(knownAtFive.Prepared.NormalizedWindows, static window =>
            Assert.Equal(TemporalRangeEndStatus.OpenAtHorizon, window.Range.EndStatus));
        Assert.Contains(knownAtFive.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.FutureWindowExcluded);
    }

    [Fact]
    public void KnownAtExcludesFutureSegmentWindows()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 5, period: "FirstPeriod");
        AddClosedWindow(pipeline, source: "source-a", start: 6, end: 10, period: "SecondPeriod");

        var prepared = pipeline.History
            .Compare("Known-at segmented audit")
            .Target("source-a", selector => selector.Source("source-a"))
            .Against("source-b", selector => selector.Source("source-b"))
            .Within(scope => scope.Window("SelectionPriced"))
            .Normalize(normalization => normalization.KnownAtPosition(5))
            .Using(comparators => comparators.Residual())
            .Prepare();

        var normalized = Assert.Single(prepared.NormalizedWindows);

        Assert.Contains(normalized.Segments, segment =>
            string.Equals(segment.Name, "period", StringComparison.Ordinal)
            && Equals(segment.Value, "FirstPeriod"));
        Assert.DoesNotContain(prepared.NormalizedWindows, window =>
            window.Segments.Any(segment => Equals(segment.Value, "SecondPeriod")));
        Assert.Contains(prepared.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.FutureWindowExcluded);
    }

    [Fact]
    public void KnownAtCohortActivityDoesNotUseUnavailableMembers()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 1, end: 6);
        AddClosedWindow(pipeline, source: "source-b", start: 1, end: 6);
        AddClosedWindow(pipeline, source: "source-c", start: 1, end: 11);

        var result = pipeline.History
            .Compare("Known-at cohort threshold")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.AtLeast(2)))
            .Within(scope => scope.Window("SelectionPriced"))
            .Normalize(normalization => normalization.KnownAtPosition(6))
            .Using(comparators => comparators.Residual())
            .Run();

        Assert.Equal(0, result.ResidualRows.TotalPositionLength());
    }

    [Fact]
    public void KnownAtAsOfCohortPlanDoesNotMatchUnavailableFutureMember()
    {
        var pipeline = CreatePipeline();

        AddClosedWindow(pipeline, source: "source-a", start: 10, end: 11);
        AddClosedWindow(pipeline, source: "source-b", start: 8, end: 11);
        AddClosedWindow(pipeline, source: "source-c", start: 12, end: 20);

        var result = pipeline.History
            .Compare("Known-at cohort as-of")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort.Sources("source-b", "source-c"))
            .Within(scope => scope.Window("SelectionPriced"))
            .Normalize(normalization => normalization.KnownAtPosition(11))
            .Using(comparators => comparators.AsOf(
                AsOfDirection.Next,
                TemporalAxis.ProcessingPosition,
                toleranceMagnitude: 5))
            .Run();

        var row = Assert.Single(result.AsOfRows);

        Assert.Equal(AsOfMatchStatus.NoMatch, row.Status);
        Assert.Null(row.MatchedRecordId);
        Assert.Contains(result.Diagnostics, diagnostic =>
            diagnostic.Code == ComparisonPlanValidationCode.FutureWindowExcluded);
    }

    [Fact]
    public void CohortResidualIsProvisionalWhenThresholdEvidenceIsOpen()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay"), source: "source-a");
        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay"), source: "source-b");
        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: false, "InPlay"), source: "source-a");

        var result = RunAtLeastCohortResidual(pipeline, live: true);
        var thresholdSegment = Assert.Single(
            result.ResidualRows,
            row => row.Range.Start == TemporalPoint.ForPosition(2));
        var thresholdIndex = FindResidualIndex(result.ResidualRows, thresholdSegment);
        var finality = Assert.Single(
            result.RowFinalities,
            row => row.RowId == "residual[" + thresholdIndex + "]");

        Assert.Equal(ComparisonFinality.Provisional, finality.Finality);
    }

    [Fact]
    public void CohortResidualChangelogRevisesWhenThresholdEvidenceCloses()
    {
        var pipeline = CreatePipeline();

        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay"), source: "source-a");
        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: true, "InPlay"), source: "source-b");
        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: false, "InPlay"), source: "source-a");

        var previous = RunAtLeastCohortResidual(pipeline, live: true);

        pipeline.Ingest(new PriceUpdate("selection-1", HasPrice: false, "InPlay"), source: "source-b");

        var current = RunAtLeastCohortResidual(pipeline, live: false);
        var changes = ComparisonChangelog.Create(previous.RowFinalities, current.RowFinalities);

        Assert.Contains(changes, change => change.Finality == ComparisonFinality.Revised);
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
        string period = "FinalQuarter")
    {
        var open = new WindowEmission<PriceUpdate>(
            "SelectionPriced",
            "selection-1",
            new PriceUpdate("selection-1", HasPrice: true, "InPlay"),
            WindowTransitionKind.Opened,
            source,
            Segments:
            [
                new WindowSegment("phase", "InPlay"),
                new WindowSegment("period", period, ParentName: "phase")
            ]);
        var close = open with
        {
            Event = new PriceUpdate("selection-1", HasPrice: false, "InPlay"),
            Kind = WindowTransitionKind.Closed
        };

        pipeline.History.Record([open], start, eventTime: null);
        pipeline.History.Record([close], end, eventTime: null);
    }

    private static ComparisonResult RunAtLeastCohortResidual(
        EventPipeline<PriceUpdate> pipeline,
        bool live)
    {
        var builder = pipeline.History
            .Compare("Threshold cohort residual")
            .Target("source-a", selector => selector.Source("source-a"))
            .AgainstCohort("cohort", cohort => cohort
                .Sources("source-b", "source-c")
                .Activity(CohortActivity.AtLeast(2)))
            .Within(scope => scope.Window("SelectionPriced"))
            .Using(comparators => comparators.Residual());

        return live
            ? builder.RunLive(TemporalPoint.ForPosition(5))
            : builder.Run();
    }

    private static int FindResidualIndex(
        IReadOnlyList<ResidualRow> rows,
        ResidualRow target)
    {
        for (var i = 0; i < rows.Count; i++)
        {
            if (rows[i] == target)
            {
                return i;
            }
        }

        throw new InvalidOperationException("Residual row was not found.");
    }

    private sealed record PriceUpdate(string SelectionId, bool HasPrice, string Phase);
}
