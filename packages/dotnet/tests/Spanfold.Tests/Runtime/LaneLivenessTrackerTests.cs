using Spanfold;

namespace Spanfold.Tests.Runtime;

public sealed class LaneLivenessTrackerTests
{
    [Fact]
    public void FirstObservationEmitsAliveStateAndRepeatedHeartbeatDoesNotEmit()
    {
        var startedAt = DateTimeOffset.Parse("2026-04-21T10:00:00Z");
        var tracker = LaneLivenessTracker.ForLanes(startedAt, TimeSpan.FromSeconds(30), "lane-a");

        var first = Assert.Single(tracker.Observe("lane-a", startedAt.AddSeconds(5)));
        var second = tracker.Observe("lane-a", startedAt.AddSeconds(10));

        Assert.Equal("lane-a", first.Lane);
        Assert.False(first.IsSilent);
        Assert.Equal(startedAt.AddSeconds(5), first.OccurredAt);
        Assert.Empty(second);
    }

    [Fact]
    public void CheckEmitsSilenceOnceWhenLaneExpires()
    {
        var startedAt = DateTimeOffset.Parse("2026-04-21T10:00:00Z");
        var tracker = LaneLivenessTracker.ForLanes(startedAt, TimeSpan.FromSeconds(30), "lane-a");

        tracker.Observe("lane-a", startedAt.AddSeconds(5));

        var early = tracker.Check(startedAt.AddSeconds(34));
        var expired = Assert.Single(tracker.Check(startedAt.AddSeconds(40)));
        var repeated = tracker.Check(startedAt.AddSeconds(50));

        Assert.Empty(early);
        Assert.True(expired.IsSilent);
        Assert.Equal(startedAt.AddSeconds(35), expired.OccurredAt);
        Assert.Equal(startedAt.AddSeconds(40), expired.EvaluatedAt);
        Assert.Empty(repeated);
    }

    [Fact]
    public void ObservationAfterSilenceEmitsRecovery()
    {
        var startedAt = DateTimeOffset.Parse("2026-04-21T10:00:00Z");
        var tracker = LaneLivenessTracker.ForLanes(startedAt, TimeSpan.FromSeconds(30), "lane-a");

        tracker.Observe("lane-a", startedAt);
        tracker.Check(startedAt.AddSeconds(31));

        var recovery = Assert.Single(tracker.Observe("lane-a", startedAt.AddSeconds(45)));

        Assert.False(recovery.IsSilent);
        Assert.Equal(startedAt.AddSeconds(45), recovery.OccurredAt);
    }

    [Fact]
    public void ObservationBeforeEvaluatedHorizonIsRejected()
    {
        var startedAt = DateTimeOffset.Parse("2026-04-21T10:00:00Z");
        var tracker = LaneLivenessTracker.ForLanes(startedAt, TimeSpan.FromSeconds(30), "lane-a");

        tracker.Check(startedAt.AddSeconds(100));

        var exception = Assert.Throws<ArgumentOutOfRangeException>(() =>
            tracker.Observe("lane-a", startedAt.AddSeconds(50)));

        Assert.Contains("last evaluated horizon", exception.Message);
    }

    [Fact]
    public void CheckCanEmitSilenceForLaneThatNeverReported()
    {
        var startedAt = DateTimeOffset.Parse("2026-04-21T10:00:00Z");
        var tracker = LaneLivenessTracker.ForLanes(startedAt, TimeSpan.FromSeconds(30), "lane-a");

        var signal = Assert.Single(tracker.Check(startedAt.AddSeconds(40)));

        Assert.True(signal.IsSilent);
        Assert.Equal(startedAt.AddSeconds(30), signal.OccurredAt);
        Assert.Equal(startedAt.AddSeconds(40), signal.EvaluatedAt);
    }

    [Fact]
    public void LivenessSignalsCanRecordSilenceWindows()
    {
        var startedAt = DateTimeOffset.Parse("2026-04-21T10:00:00Z");
        var tracker = LaneLivenessTracker.ForLanes(startedAt, TimeSpan.FromSeconds(30), "lane-a");
        var pipeline = Spanfold
            .For<LaneLivenessSignal>()
            .RecordWindows()
            .WithEventTime(signal => signal.OccurredAt)
            .TrackWindow("LaneSilent", window => window
                .Key(signal => signal.Lane)
                .ActiveWhen(signal => signal.IsSilent)
                .Tag("threshold", signal => signal.SilenceThreshold));

        foreach (var signal in tracker.Observe("lane-a", startedAt))
        {
            pipeline.Ingest(signal, source: "liveness");
        }

        foreach (var signal in tracker.Check(startedAt.AddSeconds(31)))
        {
            pipeline.Ingest(signal, source: "liveness");
        }

        foreach (var signal in tracker.Observe("lane-a", startedAt.AddSeconds(45)))
        {
            pipeline.Ingest(signal, source: "liveness");
        }

        var window = Assert.Single(pipeline.History.Query()
            .Window("LaneSilent")
            .Key("lane-a")
            .ClosedWindows());

        Assert.Equal(startedAt.AddSeconds(30), window.StartTime);
        Assert.Equal(startedAt.AddSeconds(45), window.EndTime);
        Assert.Equal("liveness", window.Source);
    }

    [Fact]
    public void TrackerRejectsUnknownLane()
    {
        var startedAt = DateTimeOffset.Parse("2026-04-21T10:00:00Z");
        var tracker = LaneLivenessTracker.ForLanes(startedAt, TimeSpan.FromSeconds(30), "lane-a");

        Assert.Throws<ArgumentException>(() => tracker.Observe("lane-b", startedAt));
    }
}
