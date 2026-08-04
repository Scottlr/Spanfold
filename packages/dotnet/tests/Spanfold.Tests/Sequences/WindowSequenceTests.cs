using Spanfold;
using Spanfold.Sequences;

namespace Spanfold.Tests.Sequences;

public sealed class WindowSequenceTests
{
    [Fact]
    public void ThreeStepJourneyUsesOnsetOrderAndEnvelopeTiming()
    {
        var history = History(
            Closed("Warning", 0, 10),
            Closed("Offline", 3, 6),
            Closed("Recovered", 7, 8));

        var result = history.MatchSequence("incident journey")
            .Step("Warning")
            .Then("Offline")
            .Then("Recovered")
            .WithMaximumGap(1)
            .Run();

        var match = Assert.Single(result.Matches);
        Assert.Equal(["Warning", "Offline", "Recovered"], match.Evidence.Select(item => item.Window.WindowName));
        Assert.Equal(0, match.StartPosition);
        Assert.Equal(10, match.EndPosition);
        Assert.Equal(10, match.EndToEndPositionMagnitude);
        Assert.Equal(1, match.TotalGapPositionMagnitude);
        Assert.Equal(ComparisonFinality.Final, match.Finality);
    }

    [Fact]
    public void EarliestCompletionConsumesEvidenceOnlyOnce()
    {
        var history = History(
            Closed("Requested", 0, 10),
            Closed("Requested", 1, 2),
            Closed("Approved", 2, 3));

        var result = history.MatchSequence("approval")
            .Step("Requested")
            .Then("Approved")
            .Run();

        var match = Assert.Single(result.Matches);
        Assert.Equal(1, match.Evidence[0].Range.Start.Position);
        Assert.Equal(2, match.Evidence[1].Range.Start.Position);
    }

    [Fact]
    public void MaximumGapIsInclusiveAndRejectsLargerGaps()
    {
        var history = History(
            Closed("Requested", 0, 1),
            Closed("Approved", 4, 5));

        var rejected = history.MatchSequence("approval")
            .Step("Requested")
            .Then("Approved")
            .WithMaximumGap(2)
            .Run();
        var accepted = history.MatchSequence("approval")
            .Step("Requested")
            .Then("Approved")
            .WithMaximumGap(3)
            .Run();

        Assert.Empty(rejected.Matches);
        Assert.Single(accepted.Matches);
    }

    [Fact]
    public void LiveHorizonClipsOpenEvidenceAndPreservesProvisionalFinality()
    {
        var history = WindowHistory.FromRecords(
            [Closed("Requested", 0, 2)],
            [new OpenWindow("Approved", "order-1", 3, Source: "provider-a", Partition: "tenant-a")]);
        var builder = history.MatchSequence("approval")
            .Step("Requested")
            .Then("Approved");

        var result = builder.RunLive(TemporalPoint.ForPosition(5));

        var match = Assert.Single(result.Matches);
        Assert.Equal(ComparisonFinality.Provisional, match.Finality);
        Assert.Equal(5, match.EndPosition);
        Assert.Equal(TemporalRangeEndStatus.OpenAtHorizon, match.Evidence[1].Range.EndStatus);
        Assert.Throws<InvalidOperationException>(() => builder.Run());
    }

    [Fact]
    public void FirstStepComparerAnchorsKeyCorrelationWithoutCrossingSourceOrPartition()
    {
        var comparers = new Dictionary<string, IEqualityComparer<object>>(StringComparer.Ordinal)
        {
            ["Requested"] = new ObjectStringComparer(StringComparer.OrdinalIgnoreCase)
        };
        var history = new WindowHistory(enabled: true, comparers);
        Record(history, "Requested", "Order-A", 0, 2, "provider-a", "tenant-a");
        Record(history, "Approved", "order-a", 3, 4, "provider-a", "tenant-a");
        Record(history, "Approved", "order-a", 3, 4, "provider-b", "tenant-a");
        Record(history, "Approved", "order-a", 3, 4, "provider-a", "tenant-b");

        var result = history.MatchSequence("approval")
            .Step("Requested")
            .Then("Approved")
            .Run();

        var match = Assert.Single(result.Matches);
        Assert.Equal("Order-A", match.Key);
        Assert.Equal("provider-a", match.Source);
        Assert.Equal("tenant-a", match.Partition);
    }

    private static WindowHistory History(params ClosedWindow[] windows)
    {
        return WindowHistory.FromRecords(windows, []);
    }

    private static ClosedWindow Closed(string name, long start, long end)
    {
        return new ClosedWindow(
            name,
            "order-1",
            start,
            end,
            Source: "provider-a",
            Partition: "tenant-a");
    }

    private static void Record(
        WindowHistory history,
        string windowName,
        string key,
        long start,
        long end,
        string source,
        string partition)
    {
        history.Record(
            [new WindowEmission<int>(windowName, key, 0, WindowTransitionKind.Opened, source, partition)],
            start,
            eventTime: null);
        history.Record(
            [new WindowEmission<int>(windowName, key, 0, WindowTransitionKind.Closed, source, partition)],
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
}
