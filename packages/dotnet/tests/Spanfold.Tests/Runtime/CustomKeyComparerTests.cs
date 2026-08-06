using Spanfold;
using Spanfold.Sequences;

namespace Spanfold.Tests.Runtime;

public sealed class CustomKeyComparerTests
{
    [Fact]
    public void WindowCanUseCustomKeyComparer()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m,
                comparer: StringComparer.OrdinalIgnoreCase)
            .Build();

        pipeline.Ingest(new PriceTick("Selection-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", 0m));

        Assert.Empty(result.Emissions);
    }

    [Fact]
    public void RecordedWindowClosesWithCustomKeyComparer()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .RecordWindows()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m,
                comparer: StringComparer.OrdinalIgnoreCase)
            .Build();

        pipeline.Ingest(new PriceTick("Selection-1", 0m));
        pipeline.Ingest(new PriceTick("selection-1", 1m));

        Assert.Empty(pipeline.History.OpenWindows);
        Assert.Single(pipeline.History.ClosedWindows);
        Assert.Single(pipeline.History.Query().Key("SELECTION-1").ClosedWindows());
    }

    [Fact]
    public void ComparisonAlignsComparerEquivalentKeysAcrossSources()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .RecordWindows()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m,
                comparer: StringComparer.OrdinalIgnoreCase)
            .Build();

        pipeline.Ingest(new PriceTick("Selection-1", 0m), source: "provider-a");
        pipeline.Ingest(new PriceTick("selection-1", 0m), source: "provider-b");
        pipeline.Ingest(new PriceTick("Selection-1", 1m), source: "provider-a");
        pipeline.Ingest(new PriceTick("selection-1", 1m), source: "provider-b");

        var result = pipeline.History.Compare("Comparer alignment")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("SelectionSuspension"))
            .Using(comparators => comparators.Overlap())
            .Run();

        Assert.Single(result.OverlapRows);
    }

    [Fact]
    public void ComparisonKeySelectorUsesConfiguredWindowComparer()
    {
        var comparers = KeyComparers();
        var history = new WindowHistory(enabled: true, comparers);
        Record(history, "State", "Device-A", 0, 5, "provider-a");

        var prepared = history.Compare("Key selection")
            .Target("selected", selector => selector.Key("device-a"))
            .Against("other", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("State"))
            .Prepare();

        var selected = Assert.Single(prepared.SelectedWindows);
        Assert.Equal("Device-A", selected.Key);
    }

    [Fact]
    public void ImportedHistoryPreservesLiveKeyIdentityAcrossAnalysisJourneys()
    {
        var comparers = KeyComparers();
        var live = new WindowHistory(enabled: true, comparers);
        Record(live, "State", "Device-A", 0, 5, "provider-a");
        Record(live, "State", "device-a", 1, 4, "provider-b");
        Record(live, "Requested", "Order-A", 10, 12, "workflow");
        Record(live, "Approved", "order-a", 13, 15, "workflow");
        var imported = WindowHistory.FromRecords(live.ClosedWindows, [], comparers);

        var liveResults = RunKeyIdentityJourneys(live);
        var importedResults = RunKeyIdentityJourneys(imported);

        Assert.Equal((1, EpisodeRelationKind.OneToOne, 1), liveResults);
        Assert.Equal(liveResults, importedResults);
    }

    [Fact]
    public void WindowUsesDefaultComparerWhenCustomComparerIsOmitted()
    {
        var pipeline = EventPipeline
            .For<PriceTick>()
            .Window(
                "SelectionSuspension",
                key: tick => tick.SelectionId,
                isActive: tick => tick.Price == 0m)
            .Build();

        pipeline.Ingest(new PriceTick("Selection-1", 0m));
        var result = pipeline.Ingest(new PriceTick("selection-1", 0m));

        var emission = Assert.Single(result.Emissions);
        Assert.Equal("selection-1", emission.Key);
    }

    private static (int Overlaps, EpisodeRelationKind RelationKind, int Sequences) RunKeyIdentityJourneys(
        WindowHistory history)
    {
        var comparison = history.Compare("Provider comparison")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("State"))
            .Using(comparators => comparators.Overlap())
            .Run();
        var episodeComparison = history.CompareEpisodes("Provider episodes")
            .Target("provider-a", selector => selector.Source("provider-a"))
            .Against("provider-b", selector => selector.Source("provider-b"))
            .Within(scope => scope.Window("State"))
            .RelateWithin(0)
            .Run();
        var sequences = history.MatchSequence("Approval")
            .Step("Requested")
            .Then("Approved")
            .Run();

        return (
            comparison.OverlapRows.Count,
            Assert.Single(episodeComparison.Relations).Kind,
            sequences.Matches.Count);
    }

    private static Dictionary<string, IEqualityComparer<object>> KeyComparers()
    {
        var comparer = new ObjectStringComparer(StringComparer.OrdinalIgnoreCase);
        return new Dictionary<string, IEqualityComparer<object>>(StringComparer.Ordinal)
        {
            ["State"] = comparer,
            ["Requested"] = comparer
        };
    }

    private static void Record(
        WindowHistory history,
        string windowName,
        string key,
        long start,
        long end,
        string source)
    {
        history.Record(
            [new WindowEmission<int>(windowName, key, 0, WindowTransitionKind.Opened, source)],
            start,
            eventTime: null);
        history.Record(
            [new WindowEmission<int>(windowName, key, 0, WindowTransitionKind.Closed, source)],
            end,
            eventTime: null);
    }

    private sealed class ObjectStringComparer(StringComparer comparer) : IEqualityComparer<object>
    {
        public new bool Equals(object? x, object? y)
        {
            return x is string left && y is string right && comparer.Equals(left, right);
        }

        public int GetHashCode(object obj)
        {
            return comparer.GetHashCode((string)obj);
        }
    }

    private sealed record PriceTick(string SelectionId, decimal Price);
}
