using Spanfold.Watermarks;

namespace Spanfold.Tests.Runtime;

public sealed class BoundedWatermarkTrackerTests
{
    [Fact]
    public void AdvanceLane_ProgressMovesBackward_PreservesMonotonicWatermark()
    {
        var tracker = new BoundedWatermarkTracker(TimeSpan.FromMinutes(2));
        var first = tracker.AdvanceLane("lane-a", AtMinute(10));
        var stale = tracker.AdvanceLane("lane-a", AtMinute(8));

        Assert.True(first.Advanced);
        Assert.Equal(AtMinute(8), first.Watermark);
        Assert.False(stale.Advanced);
        Assert.Equal(AtMinute(10), stale.Progress);
        Assert.Equal(AtMinute(8), stale.Watermark);
    }

    [Fact]
    public void Observe_EventAtWatermarkBoundary_IsAccepted()
    {
        var tracker = new BoundedWatermarkTracker(TimeSpan.FromMinutes(2));
        tracker.AdvanceLane("lane-a", AtMinute(10));

        var boundary = tracker.Observe("lane-a", "event-1", "revision-1", AtMinute(8));
        var tooLate = tracker.Observe("lane-a", "event-2", "revision-1", AtMinute(7));

        Assert.Equal(WatermarkDecisionKind.Accepted, boundary.Kind);
        Assert.Equal(WatermarkDecisionKind.Rejected, tooLate.Kind);
    }

    [Fact]
    public void AdvanceLane_IndependentLanes_ReleaseOnlyTheirOwnBufferedEvents()
    {
        var tracker = new BoundedWatermarkTracker(TimeSpan.FromMinutes(1));
        tracker.Observe("lane-a", "event-a", "revision-1", AtMinute(5));
        tracker.Observe("lane-b", "event-b", "revision-1", AtMinute(5));

        var laneA = tracker.AdvanceLane("lane-a", AtMinute(5));
        var released = Assert.Single(laneA.Released);
        Assert.Equal("lane-a", released.Revision.LaneId);
        Assert.Equal("event-a", released.Revision.EventId);

        var laneB = tracker.AdvanceLane("lane-b", AtMinute(5));
        Assert.Equal("event-b", Assert.Single(laneB.Released).Revision.EventId);
    }

    [Fact]
    public void AdvanceLane_BufferedEvents_ReleasesInDeterministicOrder()
    {
        var tracker = new BoundedWatermarkTracker(TimeSpan.FromMinutes(1));
        tracker.Observe("lane-a", "event-b", "revision-2", AtMinute(5));
        tracker.Observe("lane-a", "event-a", "revision-2", AtMinute(5));
        tracker.Observe("lane-a", "event-a", "revision-1", AtMinute(5));
        tracker.Observe("lane-a", "event-c", "revision-1", AtMinute(4));

        var advance = tracker.AdvanceLane("lane-a", AtMinute(5));

        Assert.Collection(
            advance.Released,
            decision => Assert.Equal("event-c/revision-1", Format(decision)),
            decision => Assert.Equal("event-a/revision-1", Format(decision)),
            decision => Assert.Equal("event-a/revision-2", Format(decision)),
            decision => Assert.Equal("event-b/revision-2", Format(decision)));
    }

    [Fact]
    public void AdvanceLane_BufferedEventNowBehindWatermark_IsRejected()
    {
        var tracker = new BoundedWatermarkTracker(TimeSpan.FromMinutes(1));
        tracker.Observe("lane-a", "event-1", "revision-1", AtMinute(5));

        var advance = tracker.AdvanceLane("lane-a", AtMinute(10));

        var released = Assert.Single(advance.Released);
        Assert.Equal(WatermarkDecisionKind.Rejected, released.Kind);
        Assert.Equal(AtMinute(9), released.Watermark);
    }

    [Fact]
    public void Observe_RetainedLateRevision_EmitsStableCorrectionAndRetraction()
    {
        var tracker = new BoundedWatermarkTracker(TimeSpan.FromMinutes(2));
        tracker.AdvanceLane("lane-a", AtMinute(10));
        tracker.Observe("lane-a", "event-1", "revision-1", AtMinute(9));
        tracker.AdvanceLane("lane-a", AtMinute(11));

        var correction = tracker.Observe("lane-a", "event-1", "revision-2", AtMinute(8));

        Assert.Equal(WatermarkDecisionKind.Corrected, correction.Kind);
        Assert.Equal(
            new WatermarkRevisionReference("lane-a", "event-1", "revision-2"),
            correction.Correction!.Replacement);
        Assert.Equal(
            new WatermarkRevisionReference("lane-a", "event-1", "revision-1"),
            correction.Correction.Retraction);

        var nextCorrection = tracker.Observe(
            "lane-a",
            "event-1",
            "revision-3",
            AtMinute(8));

        Assert.Equal(
            new WatermarkRevisionReference("lane-a", "event-1", "revision-2"),
            nextCorrection.Correction!.Retraction);
    }

    [Fact]
    public void Observe_RevisionOutsideCorrectionHorizon_IsRejected()
    {
        var tracker = new BoundedWatermarkTracker(TimeSpan.FromMinutes(2));
        tracker.AdvanceLane("lane-a", AtMinute(10));
        tracker.Observe("lane-a", "event-1", "revision-1", AtMinute(9));
        tracker.AdvanceLane("lane-a", AtMinute(14));

        var expired = tracker.Observe("lane-a", "event-1", "revision-2", AtMinute(9));

        Assert.Equal(WatermarkDecisionKind.Rejected, expired.Kind);
        Assert.Null(expired.Correction);
    }

    [Fact]
    public void Constructor_NegativeAllowedLateness_IsRejected()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new BoundedWatermarkTracker(TimeSpan.FromTicks(-1)));
    }

    private static DateTimeOffset AtMinute(int minute)
    {
        return new DateTimeOffset(2026, 1, 1, 0, minute, 0, TimeSpan.Zero);
    }

    private static string Format(WatermarkDecision decision)
    {
        return $"{decision.Revision.EventId}/{decision.Revision.RevisionId}";
    }
}
