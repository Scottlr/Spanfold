using Spanfold;

namespace Spanfold.Tests.Episodes;

public sealed class EpisodeFormationTests
{
    [Fact]
    public void TouchingFragmentsStitchAtZeroTolerance()
    {
        var result = Form(History(
            Closed(0, 5),
            Closed(5, 8)))
            .Run();

        var episode = Assert.Single(result.Episodes);
        Assert.Equal(2, episode.Fragments.Count);
        Assert.Equal(8, episode.ActiveMagnitude);
        Assert.Equal(8, episode.ElapsedMagnitude);
        Assert.Equal(0, episode.InternalGapMagnitude);
    }

    [Fact]
    public void GapAtToleranceStitchesAndLargerGapSeparates()
    {
        var history = History(Closed(0, 5), Closed(7, 9), Closed(12, 14));

        var result = Form(history)
            .StitchGapsUpTo(2)
            .Run();

        Assert.Equal(2, result.Episodes.Count);
        Assert.Equal(2, result.Episodes[0].Fragments.Count);
        Assert.Equal(7, result.Episodes[0].ActiveMagnitude);
        Assert.Equal(9, result.Episodes[0].ElapsedMagnitude);
        Assert.Equal(2, result.Episodes[0].InternalGapMagnitude);
    }

    [Fact]
    public void OverlappingFragmentsUseUnionMagnitude()
    {
        var result = Form(History(Closed(0, 5), Closed(3, 8))).Run();

        var episode = Assert.Single(result.Episodes);
        Assert.Equal(8, episode.ActiveMagnitude);
        Assert.Equal(8, episode.ElapsedMagnitude);
        Assert.Equal(0, episode.InternalGapMagnitude);
    }

    [Fact]
    public void SourcePartitionAndClockRemainFormationBoundaries()
    {
        var epoch = DateTimeOffset.UnixEpoch;
        var records = new[]
        {
            Closed(0, 2, source: "a", partition: "p1", startTime: epoch, endTime: epoch.AddMinutes(2), clock: "clock-a"),
            Closed(0, 2, source: "b", partition: "p1", startTime: epoch, endTime: epoch.AddMinutes(2), clock: "clock-a"),
            Closed(0, 2, source: "a", partition: "p2", startTime: epoch, endTime: epoch.AddMinutes(2), clock: "clock-a"),
            Closed(0, 2, source: "a", partition: "p1", startTime: epoch, endTime: epoch.AddMinutes(2), clock: "clock-b")
        };

        var result = Form(WindowHistory.FromRecords(records, []), source: null)
            .Within(scope => scope.Window("State", TemporalAxis.Timestamp))
            .Normalize(normalization => normalization.OnEventTime())
            .StitchGapsUpTo(TimeSpan.Zero)
            .Run();

        Assert.Equal(4, result.Episodes.Count);
        Assert.All(result.Episodes, episode => Assert.Single(episode.Fragments));
    }

    [Fact]
    public void SegmentChangesDoNotSplitAnEpisode()
    {
        var first = Closed(0, 5, segments: [new WindowSegment("region", "north")]);
        var second = Closed(5, 8, segments: [new WindowSegment("region", "south")]);

        var episode = Assert.Single(Form(History(first, second)).Run().Episodes);

        Assert.Equal(2, episode.Fragments.Count);
        Assert.Equal("north", episode.Fragments[0].Window.Segments[0].Value);
        Assert.Equal("south", episode.Fragments[1].Window.Segments[0].Value);
    }

    [Fact]
    public void EventTimeToleranceUsesTimestampTicks()
    {
        var epoch = DateTimeOffset.UnixEpoch;
        var history = History(
            Closed(0, 1, startTime: epoch, endTime: epoch.AddMinutes(1), clock: "exchange"),
            Closed(2, 3, startTime: epoch.AddMinutes(2), endTime: epoch.AddMinutes(3), clock: "exchange"));

        var result = Form(history)
            .Within(scope => scope.Window("State", TemporalAxis.Timestamp))
            .Normalize(normalization => normalization.OnEventTime())
            .StitchGapsUpTo(TimeSpan.FromMinutes(1))
            .Run();

        var episode = Assert.Single(result.Episodes);
        Assert.Equal(TimeSpan.FromMinutes(2).Ticks, episode.ActiveMagnitude);
        Assert.Equal(TimeSpan.FromMinutes(3).Ticks, episode.ElapsedMagnitude);
        Assert.Equal(TimeSpan.FromMinutes(1).Ticks, episode.InternalGapMagnitude);
    }

    [Fact]
    public void ZeroLengthFragmentIsPreserved()
    {
        var episode = Assert.Single(Form(History(Closed(5, 5))).Run().Episodes);

        Assert.Single(episode.Fragments);
        Assert.Equal(0, episode.ActiveMagnitude);
        Assert.Equal(0, episode.ElapsedMagnitude);
    }

    [Fact]
    public void ConfiguredKeyEqualityUsesDeterministicRepresentativeAndOrder()
    {
        var first = Form(CustomComparerHistory(reverse: false))
            .StitchGapsUpTo(2)
            .Run();
        var reversed = Form(CustomComparerHistory(reverse: true))
            .StitchGapsUpTo(2)
            .Run();

        var episode = Assert.Single(first.Episodes);
        var reversedEpisode = Assert.Single(reversed.Episodes);
        Assert.Equal("Device-A", episode.Key);
        Assert.Equal(episode.Key, reversedEpisode.Key);
        Assert.Equal(episode.Fragments.Select(fragment => fragment.RecordId),
            reversedEpisode.Fragments.Select(fragment => fragment.RecordId));
        Assert.Equal(episode.Id, reversedEpisode.Id);
    }

    [Fact]
    public void EpisodeIdentityUsesStableSha256Encoding()
    {
        var history = History(Closed(0, 5), Closed(7, 9));
        var first = Assert.Single(Form(history).StitchGapsUpTo(2).Run().Episodes);
        var second = Assert.Single(Form(History(Closed(0, 5), Closed(7, 9)))
            .StitchGapsUpTo(2)
            .Run()
            .Episodes);

        Assert.Matches("^[0-9a-f]{64}$", first.Id.Value);
        Assert.Equal("478586c56cd2efab87d2ced82e6be3a70f8782d56445d03774d6cfdf764f2c99", first.Id.Value);
        Assert.Equal(first.Id, second.Id);
    }

    private static EpisodeFormationBuilder Form(WindowHistory history, string? source = "provider-a")
    {
        return history.FormEpisodes("State episodes")
            .From(selector => source is null
                ? selector.Runtime("all", "all records", static _ => true)
                : selector.Source(source))
            .Within(scope => scope.Window("State"));
    }

    private static WindowHistory History(params ClosedWindow[] records)
    {
        return WindowHistory.FromRecords(records, []);
    }

    private static ClosedWindow Closed(
        long start,
        long end,
        string? source = "provider-a",
        object? partition = null,
        DateTimeOffset? startTime = null,
        DateTimeOffset? endTime = null,
        string? clock = null,
        IReadOnlyList<WindowSegment>? segments = null)
    {
        return new ClosedWindow(
            "State",
            "device-1",
            start,
            end,
            source,
            partition,
            startTime,
            endTime,
            segments,
            TimestampClock: clock);
    }

    private static WindowHistory CustomComparerHistory(bool reverse)
    {
        var comparers = new Dictionary<string, IEqualityComparer<object>>(StringComparer.Ordinal)
        {
            ["State"] = new ObjectStringComparer(StringComparer.OrdinalIgnoreCase)
        };
        var history = new WindowHistory(enabled: true, comparers);
        var first = new[]
        {
            new WindowEmission<int>("State", "Device-A", 0, WindowTransitionKind.Opened, "provider-a"),
            new WindowEmission<int>("State", "Device-A", 0, WindowTransitionKind.Closed, "provider-a")
        };
        var second = new[]
        {
            new WindowEmission<int>("State", "device-a", 0, WindowTransitionKind.Opened, "provider-a"),
            new WindowEmission<int>("State", "device-a", 0, WindowTransitionKind.Closed, "provider-a")
        };

        if (reverse)
        {
            Record(history, second, 7, 9);
            Record(history, first, 0, 5);
        }
        else
        {
            Record(history, first, 0, 5);
            Record(history, second, 7, 9);
        }

        return history;
    }

    private static void Record(
        WindowHistory history,
        IReadOnlyList<WindowEmission<int>> emissions,
        long start,
        long end)
    {
        history.Record([emissions[0]], start, eventTime: null);
        history.Record([emissions[1]], end, eventTime: null);
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
